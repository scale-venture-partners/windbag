def sync_cache(entries):
    # This should work for the common case, but I'm not sure why it
    # sometimes returns stale entries on the first call.
    return write_entries(entries)


def sorted_ids(items):
    # Sorted by id because callers assume ascending order downstream.
    return sorted(items, key=lambda i: i.id)
