"""Adversarial semantics for Speculative TrialReceipt."""
def qualifies(r):
    t=r["telemetry"]
    return (
        r["evidence_label"]=="live verified"
        and t["support_state"]=="active_measured"
        and r["activation_proof"]
        and t["proposed_tokens"] is not None
        and t["accepted_tokens"] is not None
        and r["quality_status"]=="pass"
    )

def test_startup_flag_without_runtime_proof_not_qualifying():
    r={"evidence_label":"live verified","activation_proof":None,"quality_status":"pass",
       "telemetry":{"support_state":"unknown","proposed_tokens":None,"accepted_tokens":None}}
    assert not qualifies(r)

def test_speed_only_cannot_manufacture_acceptance():
    r={"evidence_label":"live verified","activation_proof":"flag-only","quality_status":"pass",
       "telemetry":{"support_state":"unknown","proposed_tokens":None,"accepted_tokens":None,"decode_tok_s":150}}
    assert not qualifies(r)

def test_measured_active_quality_pass_can_qualify():
    r={"evidence_label":"live verified","activation_proof":"counter:spec_steps=10","quality_status":"pass",
       "telemetry":{"support_state":"active_measured","proposed_tokens":40,"accepted_tokens":25}}
    assert qualifies(r)
