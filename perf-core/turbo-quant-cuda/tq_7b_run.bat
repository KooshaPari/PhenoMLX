@echo off
set TQ_MODEL_ID=Qwen/Qwen2.5-7B-Instruct
set TQ_N_LAYERS=28
set TQ_HF_HOME=E:\hf_cache
"C:\Users\koosh\AppData\Local\Programs\Python\Python311\python.exe" C:\bench\tq_ab_bench_v2.py C:\bench\tq_7b_results.json > C:\bench\tq_7b_run.log 2>&1
