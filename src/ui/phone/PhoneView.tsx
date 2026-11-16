import { useState } from "react";
import { api } from "../../api/client";
import { t } from "../../i18n";
import { useApp } from "../state/AppContext";

/** The QR code as a picture. It is never put into the page as HTML. */
export function qrImageSrc(svg: string): string {
  return `data:image/svg+xml;charset=utf-8,${encodeURIComponent(svg)}`;
}

/** "Telefoon koppelen": a code to scan, and "Koppeling verbreken". */
export function PhoneView() {
  const { showError } = useApp();
  const [qrSvg, setQrSvg] = useState<string | null>(null);
  const [isUnpaired, setIsUnpaired] = useState(false);

  const makeCode = async () => {
    try {
      setIsUnpaired(false);
      setQrSvg((await api.pairPhone()).qrSvg);
    } catch (e: unknown) {
      showError(e);
    }
  };

  const unpair = async () => {
    try {
      await api.unpairPhone();
      setQrSvg(null);
      setIsUnpaired(true);
    } catch (e: unknown) {
      showError(e);
    }
  };

  return (
    <section className="page phone">
      <h2>{t("phone.title")}</h2>
      <p>{t("phone.text")}</p>
      {qrSvg && <img className="qr" src={qrImageSrc(qrSvg)} alt={t("phone.qrAlt")} />}
      <div className="button-row">
        <button type="button" className="primary" onClick={makeCode}>{t("phone.make")}</button>
        <button type="button" onClick={unpair}>{t("phone.unpair")}</button>
      </div>
      {isUnpaired && <p role="status">{t("phone.unpaired")}</p>}
    </section>
  );
}
