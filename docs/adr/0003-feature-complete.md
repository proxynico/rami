# rami is feature-complete

Decided 2026-07-19, at the close of the audit queue (#17–#26, PRs #27–#35).
The tracker is empty, the refresh lifecycle is testable off-macOS, and every
module the app was asked to grow — CPU, GPU, process rows, the memory-history
row — has shipped. rami stops here.

What this means in practice:

- **Bug fixes and accuracy corrections are always in scope.** So are
  dependency updates and build hygiene.
- **New features are closed by default.** A proposal must argue at ADR level
  why it belongs in a single-glance menu bar monitor — against ADR-0001's
  bound (one status item, no dashboard creep, history limited to its single
  memory row) and ADR-0002's visual restraint — and be accepted by Nico
  before any implementation starts. "Wouldn't it be nice" does not clear
  this bar.
- **No slim-down either.** The full module set measured as negligible during
  the closing audit, and every module is user-hideable in Settings. Repeat the
  repository's runtime health check in `BUILDING.md` before using performance
  as a reason to remove behavior; exact figures without the machine, settings,
  interval, and sampled action are not comparable. Deleting shipped, tested
  code to make the app feel smaller spends risk to buy nothing; density concerns
  are handled with the existing toggles, then with shipped defaults, never with
  deletions.

Context for the decision: after the audit landed, the app read as larger than
originally intended and subjectively slower. Measurement contradicted the
slowness, and the growth was traced to deliberate, requested features rather
than drift. The remedy chosen was to freeze scope, not to unwind work.
