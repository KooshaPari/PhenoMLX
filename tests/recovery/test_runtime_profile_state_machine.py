"""Cross-object PhenoMLX profile/runtime state-machine prototype."""
from __future__ import annotations
from dataclasses import dataclass, replace
from enum import Enum
from hashlib import sha256
import json


def digest(v): return sha256(json.dumps(v,sort_keys=True,separators=(",",":")).encode()).hexdigest()


class StreamState(str,Enum):
    OPEN="open"; COMPLETE="complete"; PARTIAL_FAILED="partial_failed"; CANCELLED="cancelled"


@dataclass(frozen=True)
class Profile:
    model:str; engine:str; generation:int; cache_domain:str
    engine_version:str|None=None
    @property
    def id(self): return digest(self.__dict__)


@dataclass(frozen=True)
class RequestReceipt:
    request_id:str; realized_profile_id:str; generation:int; state:StreamState; text:str


@dataclass(frozen=True)
class CapacitySnapshot:
    free_bytes:int; required_bytes:int
    @property
    def admits(self): return self.free_bytes>=self.required_bytes


@dataclass(frozen=True)
class Support:
    supported:bool; lifecycle:str


def realize(requested_engine:str, fallback_engine:str|None, model:str, generation:int=1)->Profile:
    engine=fallback_engine or requested_engine
    return Profile(model,engine,generation,f"{engine}:default")


def test_fallback_changes_realized_identity():
    a=realize("vllm",None,"m")
    b=realize("vllm","llamacpp","m")
    assert a.id!=b.id


def test_inflight_receipt_stays_on_old_generation_after_hot_swap():
    a=Profile("m","sglang",1,"d")
    receipt=RequestReceipt("r",a.id,a.generation,StreamState.OPEN,"")
    b=replace(a,generation=2)
    assert receipt.realized_profile_id==a.id
    assert receipt.realized_profile_id!=b.id


def test_partial_stream_is_not_complete():
    r=RequestReceipt("r","p",1,StreamState.PARTIAL_FAILED,"partial")
    assert r.state is not StreamState.COMPLETE


def test_supported_profile_can_reject_current_admission():
    support=Support(True,"supported")
    cap=CapacitySnapshot(4,8)
    assert support.supported and not cap.admits


def test_cache_domain_change_changes_profile_identity():
    a=Profile("m","vllm",1,"tenant-a")
    b=replace(a,cache_domain="tenant-b")
    assert a.id!=b.id


def test_deprecated_support_does_not_erase_profile_identity():
    p=Profile("m","vllm",1,"d")
    old=p.id
    s=Support(False,"withdrawn")
    assert not s.supported and p.id==old
