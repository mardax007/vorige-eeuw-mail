// In-memory mock backend. Answers every command when the app runs without Tauri
// (plain `npm run dev` in a browser, and the tests). It never touches a real mailbox.

import type { CommandArgs, CommandHandlers, CommandName, CommandResult } from "./commands";
import { createAppHandlers } from "./mock/appHandlers";
import { createBackupHandlers } from "./mock/backupHandlers";
import { clearComposeTimers, createComposeHandlers } from "./mock/composeHandlers";
import { createSampleState } from "./mock/data";
import { MockEvents } from "./mock/events";
import { createImportHandlers } from "./mock/importHandlers";
import { createMailHandlers } from "./mock/mailHandlers";
import type { MockState, MockStore } from "./mock/state";

export class MockBackend {
  readonly events = new MockEvents();
  private state: MockState;
  private counter = 1000;
  private timers = new Set<ReturnType<typeof setTimeout>>();
  private readonly handlers: CommandHandlers;

  constructor(state: MockState = createSampleState()) {
    this.state = state;
    const store: MockStore = {
      get: () => this.state,
      set: (update) => {
        this.state = update(this.state);
      },
      nextId: (prefix) => `${prefix}-${++this.counter}`,
      events: this.events,
      later: (ms, run) => {
        const timer = setTimeout(() => {
          this.timers.delete(timer);
          run();
        }, ms);
        this.timers.add(timer);
        return timer;
      },
      cancel: (timer) => {
        clearTimeout(timer);
        this.timers.delete(timer);
      },
    };
    this.handlers = {
      ...createMailHandlers(store),
      ...createComposeHandlers(store),
      ...createAppHandlers(store),
      ...createImportHandlers(store),
      ...createBackupHandlers(store),
    };
  }

  async call<K extends CommandName>(cmd: K, args: CommandArgs<K>): Promise<CommandResult<K>> {
    const handler = this.handlers[cmd] as (
      a: CommandArgs<K>,
    ) => CommandResult<K> | Promise<CommandResult<K>>;
    const result = await handler(structuredClone(args));
    return result === undefined ? result : structuredClone(result);
  }

  /** Stops every pending timer and listener. */
  dispose(): void {
    this.timers.forEach((t) => clearTimeout(t));
    this.timers.clear();
    this.events.clear();
    clearComposeTimers();
  }

  /** For tests: a look at the current state. */
  snapshot(): MockState {
    return structuredClone(this.state);
  }
}

let instance: MockBackend | null = null;

export function getMockBackend(): MockBackend {
  if (instance === null) instance = new MockBackend();
  return instance;
}

/** For tests: start again from the sample data. */
export function resetMockBackend(state?: MockState): MockBackend {
  instance?.dispose();
  instance = new MockBackend(state);
  return instance;
}
