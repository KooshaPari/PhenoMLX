"""Architecture-gate prototype for typed PhenoMLX profiles.

No engine/model execution. This tests identity semantics the existing backend adapters
currently do not preserve.
"""
from __future__ import annotations
from dataclasses import dataclass, replace
from hashlib import sha256
import json


def digest(v: object) -> str:
    return sha256(json.dumps(v, sort_keys=True, separators=(",", ":")).encode()).hexdigest()


@dataclass(frozen=True)
class RequestedProfile:
    model: str
    engine: str
    cache_policy: str


@dataclass(frozen=True)
class RealizedProfile:
    requested_digest: str
    model: str
    engine: str
    engine_version: str | None
    generation: int
    observability: tuple[tuple[str, str], ...]

    @property
    def id(self) -> str:
        return digest(self.__dict__)


def test_fallback_is_a_different_realized_profile() -> None:
    req = RequestedProfile("model-a", "vllm", "native")
    rd = digest(req.__dict__)
    requested = RealizedProfile(rd, "model-a", "vllm", "1", 1, (("engine", "measured"),))
    fallback = RealizedProfile(rd, "model-a", "llamacpp", "2", 1, (("engine", "measured"),))
    assert requested.id != fallback.id


def test_hot_swap_changes_generation_identity() -> None:
    p = RealizedProfile("r", "model-a", "sglang", "1", 1, (("engine", "measured"),))
    assert p.id != replace(p, generation=2).id


def test_unknown_mechanism_is_explicit_not_fabricated() -> None:
    p = RealizedProfile("r", "remote-model", "remote", None, 1, (("hardware", "unknown"),))
    assert ("hardware", "unknown") in p.observability
