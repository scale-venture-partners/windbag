resource "aws_lambda_function" "poller" {
  environment {
    variables = {
      # Was missing entirely (SCA-901): shared/state.py's record_run
      # silently no-ops without this env var being set, so no agent's
      # last-run info was ever actually persisted before this fix.
      STATE_BUCKET = aws_s3_bucket.artifacts.bucket
    }
  }
}
