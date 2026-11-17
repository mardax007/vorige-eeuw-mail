import { describe, expect, it, vi } from "vitest";
import { currentStep, pushBackStep } from "./backButton";

describe("the Android back key", () => {
  it("does nothing of ours when no screen added a step", () => {
    expect(currentStep()).toBeUndefined();
  });

  it("takes the deepest step first, and of equal depth the newest", () => {
    const page = vi.fn();
    const contact = vi.fn();
    const otherPage = vi.fn();
    const removePage = pushBackStep(page, 1);
    const removeContact = pushBackStep(contact, 2);
    const removeOther = pushBackStep(otherPage, 1);

    currentStep()?.();
    expect(contact).toHaveBeenCalledOnce();

    removeContact();
    currentStep()?.();
    expect(otherPage).toHaveBeenCalledOnce();

    removeOther();
    currentStep()?.();
    expect(page).toHaveBeenCalledOnce();

    removePage();
    expect(currentStep()).toBeUndefined();
  });

  it("removing a step twice leaves the others alone", () => {
    const keep = vi.fn();
    const removeKeep = pushBackStep(keep, 1);
    const remove = pushBackStep(vi.fn(), 2);
    remove();
    remove();
    expect(currentStep()).toBe(keep);
    removeKeep();
  });
});
