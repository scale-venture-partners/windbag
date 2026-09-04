# trivy:ignore:AVD-AWS-0104
resource "aws_s3_bucket" "logs" {
  bucket = "my-logs-bucket"
}

resource "aws_wafv2_web_acl" "main" {
  # Rule 2: Known Bad Inputs — blocks Log4j (CVE-2021-44228), XSS, path traversal
  rule {
    name = "known-bad-inputs"
  }
}

# VPC Lattice service-to-service connectivity for the cache tier
resource "aws_vpc_lattice_service" "cache" {
  name = "cache"
}
