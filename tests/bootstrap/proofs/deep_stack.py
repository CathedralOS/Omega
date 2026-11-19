"""Deep-stack thread runner for the gamma gates: derivation recursion follows
the premise depth, so the work runs on a thread with an explicitly large
stack like the production gate.
"""


import threading


def run(body):
    """Run body() on a 512 MiB-stack worker; re-raise its first failure."""
    failure = []

    def worker_body():
        try:
            body()
        except BaseException as error:
            failure.append(error)

    try:
        threading.stack_size(512 * 1024 * 1024)
    except (ValueError, RuntimeError):
        pass
    worker = threading.Thread(target=worker_body)
    worker.start()
    worker.join()
    if failure:
        raise failure[0]
