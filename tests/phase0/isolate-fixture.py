import errno
import os
from pathlib import Path
import resource
import socket
import sys
import time

FILE_BYTES = 64 * 1024


def check_visibility(canary: str, host_netns: str, host_port: str) -> None:
    try:
        Path(canary).read_bytes()
    except FileNotFoundError:
        pass
    else:
        raise AssertionError("host canary is visible")
    assert "FERRUM_PHASE0_CANARY" not in os.environ
    assert os.readlink("/proc/self/ns/net") != host_netns
    assert [name for _, name in socket.if_nameindex()] == ["lo"]
    try:
        socket.create_connection(("127.0.0.1", int(host_port)), timeout=0.5).close()
    except ConnectionRefusedError:
        pass
    else:
        raise AssertionError("sandbox can reach the host's loopback listener")
    assert os.statvfs("/usr").f_flag & os.ST_RDONLY
    assert resource.getrlimit(resource.RLIMIT_STACK) == (8 * 1024 * 1024,) * 2
    assert resource.getrlimit(resource.RLIMIT_NOFILE) == (64, 64)
    Path("scratch").write_bytes(b"ok")
    print(f"ferrum:{input()}")


def check_file_limit(fd: int) -> None:
    assert os.write(fd, b"x" * FILE_BYTES) == FILE_BYTES
    try:
        os.write(fd, b"x")
    except OSError as error:
        assert error.errno == errno.EFBIG
    else:
        raise AssertionError("file crossed its byte limit")


def check_workspace_quota(case: str) -> None:
    data = b"x" * FILE_BYTES if case == "blocks" else b""
    attempts = 80 if case == "blocks" else 150
    for index in range(attempts):
        try:
            Path(f"scratch-{index}").write_bytes(data)
        except OSError as error:
            assert error.errno == errno.EDQUOT
            print(f"{case}: EDQUOT at file {index + 1}")
            return
    raise AssertionError(f"workspace {case} quota was not enforced")


case = sys.argv[1]
if case == "visibility":
    check_visibility(*sys.argv[2:])
elif case == "cpu":
    deadline = time.monotonic() + 2
    while time.monotonic() < deadline:
        pass
elif case == "wall":
    time.sleep(3)
elif case == "memory":
    memory = bytearray(160 * 1024 * 1024)
    for offset in range(0, len(memory), 4096):
        memory[offset] = 1
elif case == "processes":
    try:
        pid = os.fork()
    except OSError as error:
        assert error.errno == errno.EAGAIN
    else:
        if pid == 0:
            os._exit(0)
        os.waitpid(pid, 0)
        raise AssertionError("a second process was allowed")
elif case == "file":
    with open("large-file", "wb") as output:
        check_file_limit(output.fileno())
elif case in ("stdout", "stderr"):
    check_file_limit(1 if case == "stdout" else 2)
elif case in ("blocks", "inodes"):
    check_workspace_quota(case)
elif case == "terminate":
    pid = os.fork()
    if pid == 0:
        time.sleep(8)
        os._exit(0)
    os.waitpid(pid, 0)
else:
    raise ValueError(f"unknown Phase 0 fixture: {case}")
