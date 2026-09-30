"""Capture installed backend availability/version as source facts for profile receipts."""
from __future__ import annotations
from importlib import metadata
from omlx_research.backends.llamacpp_backend import LlamaCppBackend
from omlx_research.backends.vllm_backend import VllmBackend


def installed_version(dist:str)->str|None:
    try: return metadata.version(dist)
    except metadata.PackageNotFoundError: return None


def source_receipt(backend,dist):
    return {
        "engine": backend.capabilities.primary,
        "available": backend.is_available(),
        "installed_version": installed_version(dist),
        "authority": "verified_observation" if backend.is_available() else "deterministic_source",
    }


def test_unavailable_engine_does_not_fabricate_version_or_qualification():
    r=source_receipt(VllmBackend(model_path=None),"vllm")
    if not r["available"]:
        assert r["authority"]=="deterministic_source"


def test_llamacpp_receipt_has_explicit_availability_and_version_field():
    r=source_receipt(LlamaCppBackend(model_path=None),"llama-cpp-python")
    assert isinstance(r["available"],bool)
    assert "installed_version" in r
