/**
 * The typed error surface. `GenError` means HOST DATA is wrong (actionable by the
 * host) — thrown with a stable `code` and structured `details`. M10 adds
 * subclasses (RegistryError, TemplateError, …) and `GenCancelled`; internal
 * contradictions ("CycleVania bug") throw a plain `Error` instead, never a
 * `GenError`. Nothing in the pipeline is ever silently dropped.
 */

export class GenError extends Error {
  readonly code: string;
  readonly details?: Record<string, unknown>;

  constructor(code: string, message: string, details?: Record<string, unknown>) {
    super(message);
    this.name = "GenError";
    this.code = code;
    if (details !== undefined) this.details = details;
  }
}

/** Host-data errors, categorized. Each carries the base `code` + structured `details`. */
export class RegistryError extends GenError {
  constructor(code: string, message: string, details?: Record<string, unknown>) {
    super(code, message, details);
    this.name = "RegistryError";
  }
}
export class TemplateError extends GenError {
  constructor(code: string, message: string, details?: Record<string, unknown>) {
    super(code, message, details);
    this.name = "TemplateError";
  }
}
export class RequestError extends GenError {
  constructor(code: string, message: string, details?: Record<string, unknown>) {
    super(code, message, details);
    this.name = "RequestError";
  }
}
export class PlacementError extends GenError {
  constructor(code: string, message: string, details?: Record<string, unknown>) {
    super(code, message, details);
    this.name = "PlacementError";
  }
}
export class BudgetError extends GenError {
  constructor(code: string, message: string, details?: Record<string, unknown>) {
    super(code, message, details);
    this.name = "BudgetError";
  }
}
