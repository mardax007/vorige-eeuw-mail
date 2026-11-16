//! TCP and TLS connections to mail servers.
//!
//! TLS goes through rustls with the operating system's certificate store
//! (`rustls-platform-verifier`), never a built-in root list: antivirus mail scanning on family PCs
//! adds its own root to the Windows store (CLAUDE.md, Stack).
//!
//! Every stream is wrapped in a [`Tap`] that keeps the last bytes the server sent, so an error can
//! show the server's reply exactly as it came off the wire (docs/errors.md).

use std::collections::VecDeque;
use std::fmt;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};
use std::time::Duration;

use rustls::pki_types::ServerName;
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::net::TcpStream;
use tokio_rustls::TlsConnector;

use crate::error::{Code, Result, VemError};

pub const CONNECT_TIMEOUT: Duration = Duration::from_secs(20);
const TAP_KEEP_BYTES: usize = 16 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Security {
    /// TLS from the first byte (IMAP 993, SMTP 465).
    Tls,
    /// Plain first, then STARTTLS, which is then required.
    Starttls,
}

/// Which certificates to trust. Only the platform store exists in the app; the test server
/// variant is compiled only with the `test-support` feature, which the app never enables.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Trust {
    Platform,
    #[cfg(feature = "test-support")]
    AcceptAnyLocalTestServer,
}

/// Which protocol a connection is for, so errors get the right code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Proto {
    Imap,
    Smtp,
}

impl Proto {
    pub fn find_code(self) -> Code {
        match self {
            Proto::Imap => Code::ImapFind01,
            Proto::Smtp => Code::SmtpFind01,
        }
    }
    pub fn connect_code(self) -> Code {
        match self {
            Proto::Imap => Code::ImapConnect01,
            Proto::Smtp => Code::SmtpConnect01,
        }
    }
    pub fn timeout_code(self) -> Code {
        match self {
            Proto::Imap => Code::ImapConnect02,
            Proto::Smtp => Code::SmtpConnect01,
        }
    }
    pub fn tls_code(self) -> Code {
        match self {
            Proto::Imap => Code::ImapTls01,
            Proto::Smtp => Code::SmtpTls01,
        }
    }
    pub fn starttls_missing_code(self) -> Code {
        match self {
            Proto::Imap => Code::ImapTls02,
            Proto::Smtp => Code::SmtpTls02,
        }
    }
}

/// A stream the IMAP and SMTP clients can use.
pub trait MailStream: AsyncRead + AsyncWrite + Unpin + Send + fmt::Debug {}
impl<T: AsyncRead + AsyncWrite + Unpin + Send + fmt::Debug> MailStream for T {}

pub type BoxStream = Box<dyn MailStream>;

pub fn tls_config(trust: Trust) -> Result<Arc<rustls::ClientConfig>> {
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let builder = rustls::ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .map_err(|e| VemError::new(Code::AppCmd01).with_detail(e.to_string()))?;
    let config = match trust {
        Trust::Platform => {
            use rustls_platform_verifier::BuilderVerifierExt;
            builder
                .with_platform_verifier()
                .map_err(|e| VemError::new(Code::AppCmd01).with_detail(e.to_string()))?
                .with_no_client_auth()
        }
        #[cfg(feature = "test-support")]
        Trust::AcceptAnyLocalTestServer => builder
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(test_support::AcceptAny))
            .with_no_client_auth(),
    };
    Ok(Arc::new(config))
}

/// Open a TCP connection with a timeout, mapping failures to the protocol's codes.
pub async fn tcp(host: &str, port: u16, proto: Proto) -> Result<TcpStream> {
    let addrs = match tokio::time::timeout(CONNECT_TIMEOUT, tokio::net::lookup_host((host, port)))
        .await
    {
        Err(_) => return Err(VemError::new(proto.timeout_code()).with_detail(host.to_string())),
        Ok(Err(_)) => return Err(VemError::new(proto.find_code()).with_detail(host.to_string())),
        Ok(Ok(a)) => a.collect::<Vec<_>>(),
    };
    if addrs.is_empty() {
        return Err(VemError::new(proto.find_code()).with_detail(host.to_string()));
    }
    let mut last = None;
    for addr in addrs {
        match tokio::time::timeout(CONNECT_TIMEOUT, TcpStream::connect(addr)).await {
            Ok(Ok(s)) => {
                let _ = s.set_nodelay(true);
                return Ok(s);
            }
            Ok(Err(e)) => {
                last = Some(VemError::new(proto.connect_code()).with_detail(format!("{host}: {}", e.kind())))
            }
            Err(_) => last = Some(VemError::new(proto.timeout_code()).with_detail(host.to_string())),
        }
    }
    Err(last.unwrap_or_else(|| VemError::new(proto.connect_code()).with_detail(host.to_string())))
}

/// Wrap an already-open stream in TLS.
pub async fn upgrade<S>(
    stream: S,
    host: &str,
    trust: Trust,
    proto: Proto,
) -> Result<tokio_rustls::client::TlsStream<S>>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let config = tls_config(trust)?;
    let name = ServerName::try_from(host.to_string())
        .map_err(|_| VemError::new(proto.find_code()).with_detail(host.to_string()))?;
    match tokio::time::timeout(CONNECT_TIMEOUT, TlsConnector::from(config).connect(name, stream)).await {
        Ok(Ok(s)) => Ok(s),
        Ok(Err(e)) => Err(VemError::new(proto.tls_code()).with_detail(format!("{host}: {e}"))),
        Err(_) => Err(VemError::new(proto.timeout_code()).with_detail(host.to_string())),
    }
}

/// The last bytes a server sent, shared between the stream and whoever reports an error.
#[derive(Debug, Clone, Default)]
pub struct Recorder(Arc<Mutex<VecDeque<u8>>>);

impl Recorder {
    fn push(&self, bytes: &[u8]) {
        let mut buf = self.0.lock().unwrap_or_else(|p| p.into_inner());
        buf.extend(bytes);
        let over = buf.len().saturating_sub(TAP_KEEP_BYTES);
        buf.drain(..over);
    }

    pub fn clear(&self) {
        self.0.lock().unwrap_or_else(|p| p.into_inner()).clear();
    }

    fn text(&self) -> String {
        let buf = self.0.lock().unwrap_or_else(|p| p.into_inner());
        let (a, b) = buf.as_slices();
        let mut all = Vec::with_capacity(buf.len());
        all.extend_from_slice(a);
        all.extend_from_slice(b);
        String::from_utf8_lossy(&all).into_owned()
    }

    /// The last complete line the server sent, without the line ending. For IMAP this is the
    /// tagged `NO`/`BAD` line of a refused command.
    pub fn last_line(&self) -> String {
        let text = self.text();
        text.split("\r\n")
            .filter(|l| !l.is_empty())
            .last()
            .unwrap_or_default()
            .to_string()
    }

    /// The last complete SMTP reply, all of its lines (`550-…` … `550 …`), joined with CRLF
    /// exactly as sent.
    pub fn last_smtp_reply(&self) -> String {
        let text = self.text();
        let lines: Vec<&str> = text.split("\r\n").filter(|l| !l.is_empty()).collect();
        let Some(end) = lines.iter().rposition(|l| l.len() >= 4 && l.as_bytes()[3] == b' ' || l.len() == 3) else {
            return lines.last().map(|s| s.to_string()).unwrap_or_default();
        };
        let code = &lines[end][..3.min(lines[end].len())];
        let mut start = end;
        while start > 0 {
            let prev = lines[start - 1];
            if prev.len() >= 4 && prev.starts_with(code) && prev.as_bytes()[3] == b'-' {
                start -= 1;
            } else {
                break;
            }
        }
        lines[start..=end].join("\r\n")
    }
}

/// A stream that records what the server sends.
pub struct Tap<S> {
    inner: S,
    rec: Recorder,
}

impl<S> Tap<S> {
    pub fn new(inner: S, rec: Recorder) -> Self {
        Tap { inner, rec }
    }
    pub fn into_inner(self) -> S {
        self.inner
    }
}

impl<S> fmt::Debug for Tap<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Tap")
    }
}

impl<S: AsyncRead + Unpin> AsyncRead for Tap<S> {
    fn poll_read(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &mut ReadBuf<'_>) -> Poll<std::io::Result<()>> {
        let this = self.get_mut();
        let before = buf.filled().len();
        let res = Pin::new(&mut this.inner).poll_read(cx, buf);
        if let Poll::Ready(Ok(())) = &res {
            this.rec.push(&buf.filled()[before..]);
        }
        res
    }
}

impl<S: AsyncWrite + Unpin> AsyncWrite for Tap<S> {
    fn poll_write(self: Pin<&mut Self>, cx: &mut Context<'_>, data: &[u8]) -> Poll<std::io::Result<usize>> {
        Pin::new(&mut self.get_mut().inner).poll_write(cx, data)
    }
    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.get_mut().inner).poll_flush(cx)
    }
    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.get_mut().inner).poll_shutdown(cx)
    }
}

#[cfg(feature = "test-support")]
mod test_support {
    use rustls::DigitallySignedStruct;
    use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
    use rustls::pki_types::{CertificateDer, ServerName, UnixTime};

    /// Accepts any certificate. Only for the local throwaway test server in integration tests.
    #[derive(Debug)]
    pub struct AcceptAny;

    impl ServerCertVerifier for AcceptAny {
        fn verify_server_cert(
            &self,
            _: &CertificateDer<'_>,
            _: &[CertificateDer<'_>],
            _: &ServerName<'_>,
            _: &[u8],
            _: UnixTime,
        ) -> Result<ServerCertVerified, rustls::Error> {
            Ok(ServerCertVerified::assertion())
        }
        fn verify_tls12_signature(
            &self,
            _: &[u8],
            _: &CertificateDer<'_>,
            _: &DigitallySignedStruct,
        ) -> Result<HandshakeSignatureValid, rustls::Error> {
            Ok(HandshakeSignatureValid::assertion())
        }
        fn verify_tls13_signature(
            &self,
            _: &[u8],
            _: &CertificateDer<'_>,
            _: &DigitallySignedStruct,
        ) -> Result<HandshakeSignatureValid, rustls::Error> {
            Ok(HandshakeSignatureValid::assertion())
        }
        fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
            rustls::crypto::ring::default_provider()
                .signature_verification_algorithms
                .supported_schemes()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn last_line_is_exact() {
        let r = Recorder::default();
        r.push(b"* OK ready\r\na1 NO [AUTHENTICATIONFAILED] Authentication failed.\r\n");
        assert_eq!(r.last_line(), "a1 NO [AUTHENTICATIONFAILED] Authentication failed.");
    }

    #[test]
    fn smtp_multiline_reply_kept_whole() {
        let r = Recorder::default();
        r.push(b"250 OK\r\n550-5.1.1 The email account that you tried to reach does not exist.\r\n550-5.1.1  Please try\r\n550 5.1.1 double-checking  https://support.example/x\r\n");
        assert_eq!(
            r.last_smtp_reply(),
            "550-5.1.1 The email account that you tried to reach does not exist.\r\n550-5.1.1  Please try\r\n550 5.1.1 double-checking  https://support.example/x"
        );
    }

    #[test]
    fn recorder_keeps_only_tail() {
        let r = Recorder::default();
        r.push(&vec![b'x'; TAP_KEEP_BYTES * 2]);
        r.push(b"\r\nlast\r\n");
        assert_eq!(r.last_line(), "last");
    }

    #[test]
    fn platform_config_builds() {
        assert!(tls_config(Trust::Platform).is_ok());
    }
}
