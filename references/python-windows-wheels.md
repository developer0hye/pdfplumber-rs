# Python Windows wheel compatibility references

Observed: 2026-08-29.

## Primary sources

- [GitHub-hosted runners](https://docs.github.com/en/actions/reference/runners/github-hosted-runners)
  lists `windows-2025` as a Windows Server 2025 x64 hosted-runner label.
- [GitHub's Windows image configuration](https://github.com/actions/runner-images/blob/fde5e4c6e016034bcc7754e203b8e9d8ef5160c4/images/windows/scripts/build/Configure-BaseImage.ps1#L71-L73)
  sets the `LongPathsEnabled` registry value to `1` in the image used by the
  hosted runner.
- [Microsoft DUMPBIN `/DEPENDENTS`](https://learn.microsoft.com/en-us/cpp/build/reference/dependents?view=msvc-170)
  defines the option as reporting the DLLs from which the image imports
  functions.
- [Microsoft maximum path length limitation](https://learn.microsoft.com/en-us/windows/win32/fileio/maximum-file-path-limitation)
  distinguishes extended-length paths from normal long paths and documents
  the system and application opt-in requirements.
- [Python on Windows](https://docs.python.org/3/using/windows.html#removing-the-max-path-limitation)
  documents Python's long-path support when the operating-system policy is
  enabled.

## Applied pattern

The release matrix pins `windows-2025`, discovers the current x64 DUMPBIN with
VSWhere, and checks the built extension's exact PE machine, format, and direct
DLL imports. It installs that same wheel into an isolated CPython 3.13
environment, matches the installed extension digest to the wheel member, and
executes the real fixture from ordinary non-ASCII and longer-than-260-character
paths. The retained evidence records the runner's enabled long-path policy;
the result is not generalized to hosts where that policy is disabled.
