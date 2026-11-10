"""Persistence/restart prototype for PhenoMLX qualification records."""
from __future__ import annotations
from dataclasses import asdict,dataclass
import json


@dataclass(frozen=True)
class ProfileRecord:
    profile_id:str
    engine:str
    generation:int
    observability:dict[str,str]


@dataclass(frozen=True)
class SupportRecord:
    support_id:str
    profile_id:str
    lifecycle:str
    effective_at:str


def dump(v): return json.dumps(asdict(v),sort_keys=True)


def test_profile_roundtrip_preserves_unknown_observability():
    p=ProfileRecord("p","remote",4,{"hardware":"unknown","latency":"measured"})
    d=json.loads(dump(p))
    assert d["profile_id"]=="p" and d["observability"]["hardware"]=="unknown"


def test_support_withdrawal_is_separate_record_not_profile_mutation():
    p=ProfileRecord("p","vllm",1,{"engine":"measured"})
    s1=SupportRecord("s1",p.profile_id,"supported","2026-01-01T00:00:00Z")
    s2=SupportRecord("s2",p.profile_id,"withdrawn","2026-02-01T00:00:00Z")
    assert s1.profile_id==s2.profile_id==p.profile_id
    assert p.generation==1


def test_restart_can_recover_generation_identity_from_persisted_profile():
    p=ProfileRecord("p","sglang",7,{"engine":"measured"})
    d=json.loads(dump(p))
    assert d["generation"]==7
