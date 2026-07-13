output "application_id" {
  description = "ID of the Access application protecting the site."
  value       = cloudflare_zero_trust_access_application.site.id
}

output "policy_id" {
  description = "ID of the reusable allow-list policy (attach to other apps to reuse the same allow-list)."
  value       = cloudflare_zero_trust_access_policy.allow_emails.id
}

output "protected_domain" {
  description = "The hostname now gated by Access."
  value       = var.app_domain
}
