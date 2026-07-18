"""**Deprecated**: this package has been renamed to `sdsforge`.

`sdsconv` re-exports `sdsforge` unchanged so existing dependents keep working
during the migration window. Update your dependency to `sdsforge` and change
`import sdsconv` to `import sdsforge`; see docs/migration-from-sdsconv.md in
the repository for the full migration guide.
"""
from __future__ import annotations

import warnings as _warnings

_warnings.warn(
    "sdsconv has been renamed to sdsforge. "
    "`pip install sdsforge` and `import sdsforge` instead.",
    DeprecationWarning,
    stacklevel=2,
)

from sdsforge import *  # noqa: F401,F403
from sdsforge import __all__  # noqa: F401
