"""Adversarial controls for speculative qualification EXP-M1."""
def decision(predicted_speedup, realized_speedup, quality_ok, minimum, tolerance):
    if not quality_ok: return "reject_quality"
    if abs(predicted_speedup-realized_speedup)>tolerance: return "prediction_falsified"
    return "supported" if realized_speedup>=minimum else "reject_practical"

def test_speedup_with_quality_regression_rejected():
    assert decision(1.3,1.35,False,1.1,.2)=="reject_quality"

def test_bad_prediction_is_falsification_not_success():
    assert decision(1.5,.95,True,1.1,.2)=="prediction_falsified"

def test_small_real_speedup_below_practical_threshold_rejected():
    assert decision(1.05,1.04,True,1.1,.2)=="reject_practical"

def test_negative_drafter_pair_is_research_result():
    result={"acceptance_rate":.12,"realized_speedup":.71,"status":"negative"}
    assert result["status"]=="negative" and result["realized_speedup"]<1
