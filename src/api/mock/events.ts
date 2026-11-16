import type { CoreEvents } from "../types";

type Handler<E extends keyof CoreEvents> = (payload: CoreEvents[E]) => void;
type AnyHandler = (payload: never) => void;

/** A tiny event bus that stands in for Tauri events in the mock backend. */
export class MockEvents {
  private handlers: ReadonlyMap<string, readonly AnyHandler[]> = new Map();

  on<E extends keyof CoreEvents>(name: E, handler: Handler<E>): () => void {
    const list = this.handlers.get(name) ?? [];
    this.handlers = new Map(this.handlers).set(name, [...list, handler as AnyHandler]);
    return () => {
      const current = this.handlers.get(name) ?? [];
      this.handlers = new Map(this.handlers).set(
        name,
        current.filter((h) => h !== handler),
      );
    };
  }

  emit<E extends keyof CoreEvents>(name: E, payload: CoreEvents[E]): void {
    const list = (this.handlers.get(name) ?? []) as readonly Handler<E>[];
    list.forEach((h) => h(structuredClone(payload)));
  }

  clear(): void {
    this.handlers = new Map();
  }
}
