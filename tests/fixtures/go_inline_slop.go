package auth

func recordRun(bucket string) {
	// Was missing entirely (SCA-704): shared/state.go's recordRun
	// silently no-ops without this bucket being set, so no agent's
	// last-run info was ever actually persisted before this fix,
	// and the on-call rotation spent an entire weekend chasing a
	// symptom that had nothing to do with the actual root cause
	// before anyone thought to check whether writes even reached
	// the underlying store in the first place.
	writeState(bucket)
}
