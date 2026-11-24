"""Interpreter preflight shared by verification and tool restoration."""
import sys
import tarfile


REQUIREMENT = "Python 3.9.17 or newer with tarfile.data_filter is required"


def require_python():
    if sys.version_info < (3, 9, 17) or not hasattr(tarfile, "data_filter"):
        print(f"{REQUIREMENT}. Found Python {sys.version.split()[0]}.", file=sys.stderr)
        raise SystemExit(2)


if __name__ == "__main__":
    require_python()
    print(sys.executable)
