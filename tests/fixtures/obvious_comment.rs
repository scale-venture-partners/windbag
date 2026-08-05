fn tick(counter: &mut i32, retries: &mut i32) {
    // increment the counter
    *counter += 1;

    // return the count
    return_count(*counter);

    // skip the write because the cache is already warm
    skip_write();

    // increment retries, but cap at 5
    *retries += 1;

    // Sorted so the caller can assume the newest entry is first, because
    // downstream code relies on that ordering.
    sort_desc(counter);
}
