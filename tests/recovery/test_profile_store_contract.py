"""Semantic tests for the storage-neutral PhenoMLX profile history contract."""
from __future__ import annotations
from dataclasses import dataclass
from datetime import datetime, timezone


@dataclass(frozen=True)
class Event:
    kind:str; id:str; payload:tuple


class Store:
    def __init__(self): self.events={}
    def append(self,e:Event):
        old=self.events.get(e.id)
        if old is None: self.events[e.id]=e; return "appended"
        if old==e: return "idempotent"
        raise ValueError("conflicting duplicate id")


def test_duplicate_identical_event_is_idempotent():
    s=Store(); e=Event("profile","p1",(("engine","vllm"),))
    assert s.append(e)=="appended"
    assert s.append(e)=="idempotent"


def test_duplicate_id_with_different_payload_is_conflict():
    import pytest
    s=Store(); s.append(Event("profile","p1",(("engine","vllm"),)))
    with pytest.raises(ValueError): s.append(Event("profile","p1",(("engine","sglang"),)))


def test_support_withdrawal_appends_not_mutates_profile():
    s=Store()
    p=Event("profile","p1",(("generation",1),))
    s.append(p)
    s.append(Event("support","s1",(("profile_id","p1"),("lifecycle","supported"))))
    s.append(Event("support","s2",(("profile_id","p1"),("lifecycle","withdrawn"),("supersedes","s1"))))
    assert s.events["p1"]==p


def test_stale_capacity_is_not_current():
    now=datetime(2026,9,30,17,0,tzinfo=timezone.utc)
    fresh_until=datetime(2026,9,30,16,59,tzinfo=timezone.utc)
    assert fresh_until < now


def test_qualification_correction_requires_new_id():
    s=Store()
    s.append(Event("qualification","q1",(("status","failed"),)))
    import pytest
    with pytest.raises(ValueError): s.append(Event("qualification","q1",(("status","qualified"),)))
    assert s.append(Event("qualification","q2",(("status","qualified"),("supersedes","q1"))))=="appended"
