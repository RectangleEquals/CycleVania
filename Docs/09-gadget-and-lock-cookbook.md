# 09 · Gadget & lock cookbook

CycleVania is content-agnostic; this is the **example** catalog shipped in `@cyclevania/examples`
(`gadget-catalog.ts`, `lock-catalog.ts`). It doubles as a design reference: one distinct mechanical *verb*
per gadget, diegetic naming (no gear-slot nouns), each answering a lock no other gadget can.

## The 24 gadgets (one distinct verb each)

| Name | Cap | Answers | The verb |
|---|---|---|---|
| The Ardent Skyhook | `grapple` | fixed anchor points | reel yourself to a marked anchor |
| Gravemoth Filament | `rappel` | lethal drops | survive & reverse a deadly descent |
| Censer of the Second Wind | `leap` | anchorless mid-air gaps | a free aerial burst (jump+coyote+dash) |
| The Wend-Stone | `phase` | thin sealed barriers | blink through matter |
| The Waystone Beacon | `recall` | long backtracks | drop a beacon, warp back (self-made loop) |
| The Antipode Governor | `invert` | ceiling routes | flip local gravity |
| The Lodestar Coil | `magnet` | ferric rails/masses | attract/repel ferric matter |
| The Anchorite's Burden | `anchor` | weight-plates/gales | become immovably heavy |
| The Stillwater Diapason | `freeze` | liquid/gas barriers | solidify a fluid |
| The Tidewright Cistern | `siphon` | flood/basin puzzles | raise/lower/redirect liquid |
| The Quickening Bough | `cultivate` | fertile anchors | grow a climbable vine/bridge |
| The Everbrand Ferula | `ignite` | growth/ice/gas/gloom | combust & heat |
| The Sundering Charge | `shatter` | brittle/sealed barriers | destroy (may open a new drop) |
| The Voltaic Reliquary | `charge` | dormant machinery | energize/actuate |
| The Wan Lantern | `reveal` | false walls/ghost platforms | alter geometry-reality in a bubble |
| The Glossolith | `translate` | glyph/password/cipher locks | comprehend |
| The Aegis Thurible | `ward` | toxic/rad/spore fields | project a safe bubble |
| The Quiescent Metronome | `stasis` | moving crushers/platforms | stop a moving target |
| The Choral Antiphon | `resonate` | harmonic seals/sentinels | match a frequency |
| The Mockingbell | `lure` | sound-triggered gates | cast a decoy sound |
| The Chrysalis Fold | `small-form` | crawlspaces/vents | shrink |
| The Mirrorwright Oculus | `reflect` | energy shields/beams | catch & redirect energy |
| The Vivisector's Lance | `sunder-core` | **boss** shelled core | strip a boss's shell |
| The Gordian Edge | `sever` | **boss** anchoring cables | cut a boss's anchors |

Boss verbs gate only behind an *earlier-sphere* item. Synergies: `rappel`/`shatter` both convert one-way
drops into loops; `ignite`↔`freeze` are opposites over the same matter; `anchor`/`invert`/`magnet` form a
non-overlapping "force" family.

## Lock mechanics → primitives

| Mechanic | Primitive |
|---|---|
| Gadget-gated door | `rule = have(cap)` |
| Compound (A∧B / A∨B) | `and`/`or`; full `requiredCaps` surfaced for UI |
| Counted / multi-key | `count(cap, n)`; N items granting a countable cap |
| Consumable key | inventory item consumed by `/use` → sets a `flag` |
| One-way (drop/updraft/jump-pad/rope/crawlspace) | directed edge + `traversal` + `oneWay` (some re-opened by a gadget) |
| Arena lockdown (waves) | region `flag` set by clearing → exit `rule = flag(arena-N)` |
| Switch/lever, timed/sequenced/paired | virtual key grants a flag; `and` of flags for paired |
| Environmental puzzle / minigame | solving sets a flag; edge gated on it |
| Perception (false wall/ghost platform) | `rule = have(reveal|scry|spectral)`; cell collision toggled |
| Hazard field | `rule = have(ward|illume|freeze|ignite)` |
| Boss gate | terminal edge `rule = flag(boss-cleared)` |

## Affordance/challenge worked examples

- **Double-jump →** `leap`'s profile `{ grants: { reachUp: 2 }, bias: { zWeight: 0.4 } }`. In scope, the
  AreaComposer raises `zSpread` and permits `high-ledge` challenges up to `baseReach + reachUp`.
- **Lantern + invisible platform →** the `reveal` profile enables the `hidden-crossing` challenge; a
  `have("reveal")`-gated edge becomes a lava hall with ghost-platforms (`collider:"none"` until revealed).
  In the simulator, `/use wan-lantern` sets the `reveal` flag → the crossing opens.
