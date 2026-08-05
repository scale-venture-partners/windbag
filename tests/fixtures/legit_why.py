def get_latest(items):
    # Sorted by created_at DESC because the caller assumes the first row is
    # the most recent; changing this ordering silently breaks the dedup below.
    return sorted(items, key=lambda i: i.created_at, reverse=True)


def process(data):
    """
    This docstring goes on for a while about what the function does, on
    purpose, because Python docstrings should never be flagged by the
    verbose-comment length check no matter how long they run.
    """
    return data
