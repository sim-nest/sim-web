#!/usr/bin/env python3
"""Validate the dependency-free expedition projection contract source."""

from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
policy = (ROOT / "crates/sim-lib-view-wrist/src/expedition.rs").read_text()
tests = (ROOT / "crates/sim-lib-view-wrist/src/expedition_tests.rs").read_text()

for fact in ["mission", "passport", "route_lease", "endpoint_grant"]:
    assert fact in policy
for role in ["Glance", "Audible", "Notification", "LockScreen"]:
    assert f"SurfaceRole::{role}" in policy
for refusal in ["secret", "private-note", "undeclared-sensitive-field"]:
    assert refusal in policy
for semantic in ["Keyboard(String)", "Touch(String)", "VisibleExpedition"]:
    assert semantic in policy
for forbidden in [
    "Android", "Bluetooth", "Amazfit", "Halo", "pub coordinates", "pub sensor_frame",
]:
    assert forbidden not in policy, forbidden
for specimen in [
    "every_authority_factor_is_required",
    "reduced_roles_refuse_every_sensitive_class",
    "semantic_clutch_changes_visible_focus_without_device_data",
    "no_endpoint_identity_is_needed_and_all_continuity_identities_match",
]:
    assert specimen in tests
assert "[expected; 5]" in tests
assert "ExpeditionProjector.stop(expected)" in tests
print("expedition continuity projection and its four Rust specimens validated")
