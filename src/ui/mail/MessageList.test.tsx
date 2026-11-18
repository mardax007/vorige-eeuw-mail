import { render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { MessageList } from "./MessageList";

type Callback = (entries: Array<{ isIntersecting: boolean }>) => void;

/** A stand-in IntersectionObserver: the test decides when the end of the list comes into view. */
function fakeObserver() {
  const live: Callback[] = [];
  class Fake {
    private readonly cb: Callback;
    constructor(cb: Callback) {
      this.cb = cb;
    }
    observe() {
      live.push(this.cb);
    }
    disconnect() {
      const i = live.indexOf(this.cb);
      if (i >= 0) live.splice(i, 1);
    }
  }
  vi.stubGlobal("IntersectionObserver", Fake);
  return { scrollToEnd: () => [...live].forEach((cb) => cb([{ isIntersecting: true }])), watching: () => live.length };
}

function list(isLoading: boolean, onLoadMore: () => void) {
  return (
    <MessageList
      title="Postvak IN"
      items={[]}
      tags={[]}
      selectedId={null}
      isLoading={isLoading}
      emptyText=""
      hasMore
      onLoadMore={onLoadMore}
      onOpen={() => {}}
    />
  );
}

afterEach(() => vi.unstubAllGlobals());

describe("endless list", () => {
  it("loads the next messages when the end of the list scrolls into view, once per load", () => {
    const io = fakeObserver();
    const onLoadMore = vi.fn();
    const { rerender } = render(list(false, onLoadMore));
    io.scrollToEnd();
    io.scrollToEnd();
    expect(onLoadMore).toHaveBeenCalledTimes(1);
    rerender(list(true, onLoadMore));
    expect(io.watching()).toBe(0);
    rerender(list(false, onLoadMore));
    io.scrollToEnd();
    expect(onLoadMore).toHaveBeenCalledTimes(2);
  });

  it("keeps the button, for the keyboard", () => {
    fakeObserver();
    const onLoadMore = vi.fn();
    render(list(false, onLoadMore));
    screen.getByRole("button", { name: "Meer berichten laden" }).click();
    expect(onLoadMore).toHaveBeenCalledTimes(1);
  });
});
