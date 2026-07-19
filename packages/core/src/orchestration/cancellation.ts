/**
 * Cancellation — an AbortSignal-compatible token the async façade checks between
 * units so a long generation can be interrupted (e.g. the player left the area).
 */

export class GenCancelled extends Error {
  constructor(message = "generation cancelled") {
    super(message);
    this.name = "GenCancelled";
  }
}

export class CancellationToken {
  private _cancelled = false;
  private _reason = "";

  cancel(reason = "cancelled"): void {
    this._cancelled = true;
    this._reason = reason;
  }

  get cancelled(): boolean {
    return this._cancelled;
  }

  throwIfCancelled(): void {
    if (this._cancelled) throw new GenCancelled(this._reason);
  }

  /** Bridge a DOM/Node AbortSignal into a token. */
  static fromSignal(signal: { aborted: boolean; addEventListener?: (type: "abort", cb: () => void) => void }): CancellationToken {
    const t = new CancellationToken();
    if (signal.aborted) t.cancel("aborted");
    signal.addEventListener?.("abort", () => t.cancel("aborted"));
    return t;
  }
}
