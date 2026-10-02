"""Integration prototype against real PhenoMLX BackendBase subclasses.

No model load is required: constructors and static capabilities are sufficient to
prove that the proposed typed envelope preserves backend differences the current
GenerateResponse shape does not.
"""
from __future__ import annotations
from dataclasses import dataclass
from omlx_research.backends.llamacpp_backend import LlamaCppBackend
from omlx_research.backends.vllm_backend import VllmBackend


@dataclass(frozen=True)
class AdapterProfile:
    requested_engine: str
    realized_engine: str
    model_ref: str | None
    engine_version: str | None
    supports_streaming: bool
    supports_spec_decode: bool
    observability: tuple[tuple[str,str], ...]


def profile(backend, requested_engine: str, engine_version: str|None=None) -> AdapterProfile:
    caps=backend.capabilities
    return AdapterProfile(
        requested_engine=requested_engine,
        realized_engine=caps.primary,
        model_ref=getattr(backend,"model_path",None),
        engine_version=engine_version,
        supports_streaming=caps.supports_streaming,
        supports_spec_decode=caps.supports_spec_decode,
        observability=(("engine_version","measured" if engine_version else "unknown"),),
    )


def test_two_real_backend_classes_preserve_meaningful_capability_difference_without_loading_models():
    llama=profile(LlamaCppBackend(model_path=None),"llamacpp")
    vllm=profile(VllmBackend(model_path=None),"vllm")
    assert llama.realized_engine=="llamacpp"
    assert vllm.realized_engine=="vllm"
    assert llama.supports_spec_decode is False
    assert vllm.supports_spec_decode is True


def test_requested_vs_realized_fallback_is_explicit_over_real_adapter():
    fallback=profile(LlamaCppBackend(model_path=None),"vllm")
    assert fallback.requested_engine=="vllm"
    assert fallback.realized_engine=="llamacpp"


def test_unknown_engine_version_remains_unknown():
    p=profile(VllmBackend(model_path=None),"vllm")
    assert ("engine_version","unknown") in p.observability
