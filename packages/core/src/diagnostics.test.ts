import { describe, it, expect } from "vitest";
import { Diag, MemorySink, SILENT_SINK, type DiagnosticsSink, type DiagEvent } from "./diagnostics.js";

describe("diagnostics", () => {
  it("filters by configured level", () => {
    const sink = new MemorySink();
    const diag = new Diag({ level: "warn", sink });
    diag.error("e", "err");
    diag.warn("w", "warn");
    diag.info("i", "info");
    diag.debug("d", "debug");
    diag.trace("t", "trace");
    expect(sink.events.map((e) => e.level)).toEqual(["error", "warn"]);
  });

  it("passes everything through at trace level", () => {
    const sink = new MemorySink();
    const diag = new Diag({ level: "trace", sink });
    diag.error("e", "");
    diag.warn("w", "");
    diag.info("i", "");
    diag.debug("d", "");
    diag.trace("t", "");
    expect(sink.events).toHaveLength(5);
  });

  it("composes paths through child() and merges an event's own path", () => {
    const sink = new MemorySink();
    const root = new Diag({ level: "trace", sink });
    const area = root.child("reach3").child("area:r2");
    area.warn("code", "msg");
    area.warn("code2", "msg2", "space:s4");
    expect(sink.events[0]?.path).toBe("reach3/area:r2");
    expect(sink.events[1]?.path).toBe("reach3/area:r2/space:s4");
  });

  it("carries code, message, and details", () => {
    const sink = new MemorySink();
    new Diag({ level: "trace", sink }).warn("skeleton.junction-inserted", "inserted a junction", "reach0/area:r2", {
      count: 2,
    });
    const e = sink.events[0] as DiagEvent;
    expect(e.code).toBe("skeleton.junction-inserted");
    expect(e.message).toBe("inserted a junction");
    expect(e.details).toEqual({ count: 2 });
  });

  it("disables a throwing sink without propagating into the caller", () => {
    let calls = 0;
    const throwing: DiagnosticsSink = {
      emit() {
        calls++;
        throw new Error("sink boom");
      },
    };
    const diag = new Diag({ level: "trace", sink: throwing });
    expect(() => {
      diag.warn("a", "1");
      diag.warn("b", "2");
      diag.warn("c", "3");
    }).not.toThrow();
    // First emit threw and disabled the sink; later emits are skipped.
    expect(calls).toBe(1);
  });

  it("a shared broken-sink guard covers children too", () => {
    let calls = 0;
    const throwing: DiagnosticsSink = {
      emit() {
        calls++;
        throw new Error("boom");
      },
    };
    const root = new Diag({ level: "trace", sink: throwing });
    root.warn("root", "x"); // trips the guard
    const child = root.child("seg");
    expect(() => child.warn("child", "y")).not.toThrow();
    expect(calls).toBe(1); // child did not re-invoke the disabled sink
  });

  it("purity: emitting never changes a function's output (silent vs collecting)", () => {
    // A pure function that also narrates through the diag must return identically
    // regardless of sink/level.
    const compute = (x: number, diag: Diag): number => {
      diag.trace("step", "doubling");
      const y = x * 2;
      diag.debug("step", "done");
      return y;
    };
    const silent = compute(21, new Diag({ level: "warn", sink: SILENT_SINK }));
    const traced = compute(21, new Diag({ level: "trace", sink: new MemorySink() }));
    expect(silent).toBe(traced);
    expect(silent).toBe(42);
  });
});
