"""Runtime extension lifecycle/factor-isolation fixtures."""
def test_engine_upgrade_requires_requalification_when_abi_changes():
    ext={"qualified_engine":"vllm-0.30","api_abi":"spec-v3"}
    new={"engine":"vllm-0.31","api_abi":"spec-v4"}
    assert ext["api_abi"]!=new["api_abi"]

def test_compound_change_cannot_claim_single_factor_causality():
    factors={"intentional":["scheduler","kernel"],"incidental":[]}
    assert len(factors["intentional"])>1

def test_fallback_must_be_explicit():
    activation={"fallback":"forbid","proof_required":True}
    assert activation["fallback"] in {"forbid","explicit","disable"}

def test_negative_extension_state_is_retained():
    history=[{"extension":"x","state":"incompatible","engine":"vllm-0.31"}]
    assert history[0]["state"]=="incompatible"
