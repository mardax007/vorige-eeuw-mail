import { useCallback, useEffect, useRef, useState } from "react";
import { api, onEvent } from "../../api/client";
import type { LastCopy } from "../../api/types";
import { readableDate } from "../format";
import { askCamera, openAppSettings, scanCode, stopScan } from "../../api/scanner";
import { t } from "../../i18n";
import { useApp } from "../state/AppContext";

/** While a code is on screen, how often to look whether the phone has been linked. */
export const PAIR_CHECK_MS = 3000;

/** The QR code as a picture. It is never put into the page as HTML. */
export function qrImageSrc(svg: string): string {
  return `data:image/svg+xml;charset=utf-8,${encodeURIComponent(svg)}`;
}

/** "Telefoon koppelen": the PC shows a code, the phone scans it. */
export function PhoneView() {
  const { appInfo } = useApp();
  return appInfo?.isPhone ? <PhoneScan /> : <PcCode />;
}

/**
 * On the PC, once a phone is linked: mail kept only on this PC goes to the phone by itself
 * (decision 26), and when it last did.
 */
function PcCopyLine() {
  // undefined while asking, null when the phone never had the mail yet.
  const [last, setLast] = useState<LastCopy | null | undefined>(undefined);
  useEffect(() => {
    let alive = true;
    const look = () =>
      api
        .phoneLastCopy()
        .then((found) => {
          if (alive) setLast(found);
        })
        // Only a remark: without it the page still works.
        .catch(() => undefined);
    void look();
    const listening = onEvent("lan:sent", () => void look());
    return () => {
      alive = false;
      void listening.then((stop) => stop());
    };
  }, []);
  const when = last ? readableDate(new Date(last.at * 1000).toISOString()) : "";
  return (
    <>
      <p>{t("phone.pcCopy")}</p>
      {when && <p className="calm-line">{t("phone.lastCopy", { date: when })}</p>}
      {last === null && <p className="calm-line">{t("phone.noCopyYet")}</p>}
    </>
  );
}

/** On the PC: a code to scan, and "Koppeling verbreken" once a phone is linked. */
function PcCode() {
  const { showError } = useApp();
  const [qrSvg, setQrSvg] = useState<string | null>(null);
  const [isPaired, setIsPaired] = useState(false);
  const [isUnpaired, setIsUnpaired] = useState(false);
  // Goes up on every question and on every change: an older answer can no longer flip the screen.
  const request = useRef(0);
  // A failing look every few seconds is said once, not every time.
  const isPollErrorShown = useRef(false);

  const checkPaired = useCallback(
    async (isPoll = false) => {
      const token = ++request.current;
      try {
        const paired = await api.phonePaired();
        if (token !== request.current) return;
        isPollErrorShown.current = false;
        setIsPaired(paired);
      } catch (e: unknown) {
        if (token !== request.current) return;
        if (isPoll && isPollErrorShown.current) return;
        if (isPoll) isPollErrorShown.current = true;
        showError(e);
      }
    },
    [showError],
  );

  useEffect(() => {
    void checkPaired();
  }, [checkPaired]);

  // The phone links on its own after scanning; look now and then while the code is shown.
  useEffect(() => {
    if (!qrSvg || isPaired) return;
    const timer = setInterval(() => void checkPaired(true), PAIR_CHECK_MS);
    return () => clearInterval(timer);
  }, [qrSvg, isPaired, checkPaired]);

  const makeCode = async () => {
    request.current += 1;
    isPollErrorShown.current = false;
    try {
      setIsUnpaired(false);
      setQrSvg((await api.pairPhone()).qrSvg);
    } catch (e: unknown) {
      showError(e);
      return;
    }
    await checkPaired();
  };

  const unpair = async () => {
    request.current += 1;
    try {
      await api.unpairPhone();
      setQrSvg(null);
      setIsUnpaired(true);
    } catch (e: unknown) {
      showError(e);
    }
    await checkPaired();
  };

  return (
    <section className="page phone">
      <h2>{t("phone.title")}</h2>
      {/* Once the phone has scanned it, the code has done its work: say so instead. */}
      {isPaired && (
        <p className="calm-line" role="status">
          {t("phone.pcLinked")}
        </p>
      )}
      <p>{t("phone.text")}</p>
      {isPaired && <PcCopyLine />}
      {qrSvg && !isPaired && <img className="qr" src={qrImageSrc(qrSvg)} alt={t("phone.qrAlt")} />}
      <div className="button-row">
        <button type="button" className="primary" onClick={makeCode}>{t("phone.make")}</button>
        {isPaired && <button type="button" onClick={unpair}>{t("phone.unpair")}</button>}
      </div>
      {isUnpaired && <p role="status">{t("phone.unpaired")}</p>}
    </section>
  );
}

type ScanPhase = "idle" | "denied" | "scanning" | "linking" | "linked";

/**
 * On the phone: "Code scannen" reads the PC's code with the camera. The camera shows behind the
 * page, so while it runs only a hint, a frame and a big Stoppen button stay on screen.
 */
function PhoneScan() {
  const { showError } = useApp();
  const [phase, setPhase] = useState<ScanPhase>("idle");
  const [isPaired, setIsPaired] = useState(false);
  const [isUnpaired, setIsUnpaired] = useState(false);
  const isScanning = phase === "scanning";

  useEffect(() => {
    let alive = true;
    api
      .phonePaired()
      .then((paired) => {
        if (alive) setIsPaired(paired);
      })
      .catch(showError);
    return () => {
      alive = false;
    };
  }, [showError]);

  useEffect(() => {
    if (!isScanning) return;
    document.documentElement.classList.add("scanning");
    return () => {
      document.documentElement.classList.remove("scanning");
      // Leaving the page while scanning switches the camera off; a failure here has no screen left.
      void stopScan().catch(() => undefined);
    };
  }, [isScanning]);

  const scan = async () => {
    setIsUnpaired(false);
    try {
      if (!(await askCamera())) {
        setPhase("denied");
        return;
      }
      setPhase("scanning");
      const text = await scanCode();
      if (text === null) {
        setPhase("idle");
        return;
      }
      setPhase("linking");
      await api.pairWithCode(text);
      setIsPaired(true);
      setPhase("linked");
    } catch (e: unknown) {
      setPhase("idle");
      showError(e);
    }
  };

  const stop = async () => {
    try {
      await stopScan();
    } catch (e: unknown) {
      showError(e);
    }
    setPhase("idle");
  };

  const settings = async () => {
    try {
      await openAppSettings();
    } catch (e: unknown) {
      showError(e);
    }
  };

  const unpair = async () => {
    try {
      await api.unpairPhone();
      setIsPaired(false);
      setIsUnpaired(true);
      setPhase("idle");
    } catch (e: unknown) {
      showError(e);
    }
  };

  if (isScanning) {
    return (
      <div className="scan-overlay" role="dialog" aria-label={t("phone.scan")}>
        <p className="scan-hint">{t("phone.scanHint")}</p>
        <div className="scan-frame" aria-hidden="true" />
        <button type="button" className="primary scan-stop" onClick={() => void stop()}>
          {t("phone.scanStop")}
        </button>
      </div>
    );
  }

  return (
    <section className="page phone">
      <h2>{t("phone.title")}</h2>
      {isPaired && phase !== "linked" && <p>{t("phone.isLinked")}</p>}
      <p>{t("phone.scanText")}</p>
      {phase === "denied" && (
        <div className="notice-bar" role="alert">
          <span>{t("phone.cameraDenied")}</span>
          <button type="button" onClick={() => void settings()}>
            {t("phone.openSettings")}
          </button>
        </div>
      )}
      {phase === "linking" && <p role="status">{t("phone.linking")}</p>}
      {phase === "linked" && <p role="status">{t("phone.linked")}</p>}
      <div className="button-row">
        <button type="button" className="primary" disabled={phase === "linking"} onClick={() => void scan()}>
          {t("phone.scan")}
        </button>
        {isPaired && (
          <button type="button" onClick={() => void unpair()}>
            {t("phone.unpair")}
          </button>
        )}
      </div>
      {isUnpaired && <p role="status">{t("phone.unpairedPhone")}</p>}
    </section>
  );
}
