# Mature contract v0.4 delta

Acceptance-relevant runtime auto-tuning/effective configuration must be represented.

If an engine changes kernel choice, cache policy, quantization behavior, routing/model choice or other acceptance-relevant effective configuration after launch, either:
- RealizedProfile/RuntimeGeneration changes, or
- an explicit RuntimeObservationEpoch/effective-config fingerprint binds each QualificationAssessment/request evidence.

A static requested/config file is insufficient when runtime behavior materially diverges.

No other semantic change from v0.3.
