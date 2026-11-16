import { useCallback, useEffect, useRef, useState } from "react";
import { api } from "../../api/client";
import { t } from "../../i18n";
import { useApp } from "../state/AppContext";

/** While a code is on screen, how often to look whether the phone has been linked. */
export const PAIR_CHECK_MS = 3000;

/** The QR code as a picture. It is never put into the page as HTML. */
export function qrImageSrc(svg: string): string {
  return `data:image/svg+xml;charset=utf-8,${encodeURIComponent(svg)}`;
}

/** "Telefoon koppelen": a code to scan, and "Koppeling verbreken" once a phone is linked. */
export function PhoneView() {
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
      <p>{t("phone.text")}</p>
      {qrSvg && <img className="qr" src={qrImageSrc(qrSvg)} alt={t("phone.qrAlt")} />}
      <div className="button-row">
        <button type="button" className="primary" onClick={makeCode}>{t("phone.make")}</button>
        {isPaired && <button type="button" onClick={unpair}>{t("phone.unpair")}</button>}
      </div>
      {isUnpaired && <p role="status">{t("phone.unpaired")}</p>}
    </section>
  );
}
