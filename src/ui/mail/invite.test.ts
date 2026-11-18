import { describe, expect, it } from "vitest";
import { inviteWhen, zonedToDate } from "./invite";

const AMS = "Europe/Amsterdam";

describe("zonedToDate", () => {
  it("turns a clock time in a zone into the moment, summer and winter", () => {
    expect(zonedToDate("2026-11-20T14:00:00", AMS)?.toISOString()).toBe("2026-11-20T13:00:00.000Z");
    expect(zonedToDate("2026-07-01T14:00:00", AMS)?.toISOString()).toBe("2026-07-01T12:00:00.000Z");
    expect(zonedToDate("2026-11-20T09:00:00", "America/New_York")?.toISOString()).toBe("2026-11-20T14:00:00.000Z");
  });

  it("gives nothing for a zone this device does not know", () => {
    expect(zonedToDate("2026-11-20T14:00:00", "Mars Standard Time")).toBeNull();
  });
});

describe("inviteWhen", () => {
  it("names the day and both times on one day", () => {
    const start = { wall: "2026-11-20T14:00:00", zone: AMS, allDay: false };
    const end = { wall: "2026-11-20T17:00:00", zone: AMS, allDay: false };
    expect(inviteWhen(start, end, AMS)).toBe("vrijdag 20 november 2026, 14:00 tot 17:00");
  });

  it("shows a UTC time in the device's own time", () => {
    const start = { wall: "2026-12-01T09:00:00", zone: "UTC", allDay: false };
    expect(inviteWhen(start, null, AMS)).toBe("dinsdag 1 december 2026, 10:00");
  });

  it("says de hele dag, and the last day of a longer one", () => {
    const day = { wall: "2026-12-24", zone: null, allDay: true };
    expect(inviteWhen(day, { wall: "2026-12-25", zone: null, allDay: true })).toBe("donderdag 24 december 2026, de hele dag");
    expect(inviteWhen(day, { wall: "2026-12-27", zone: null, allDay: true })).toBe(
      "donderdag 24 december 2026 tot en met zaterdag 26 december 2026",
    );
  });

  it("keeps the time as written, with the zone's name, when the zone is unknown", () => {
    const start = { wall: "2026-12-01T09:00:00", zone: "Mars Standard Time", allDay: false };
    expect(inviteWhen(start, null)).toBe("dinsdag 1 december 2026, 09:00 (tijdzone Mars Standard Time)");
  });
});
