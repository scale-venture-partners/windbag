fn record_run(bucket: &str) {
    // Was missing entirely (SCA-501): shared/state.rs's record_run
    // silently no-ops without this bucket being set, so no agent's
    // last-run info was ever actually persisted before this fix.
    write_state(bucket);
}
