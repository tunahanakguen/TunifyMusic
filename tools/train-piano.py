"""Trainiert das Klavier-Modell (RVC v2, 48 kHz, RMVPE) aus den Daten von make-piano-dataset.py.

Aufruf:  engine\\.venv\\Scripts\\python -I tools\\train-piano.py [Epochen] [Batch]
Zwischenstände alle 25 Epochen: engine/vendor/applio/logs/tunify-klavier/tunify-klavier_<E>e_<S>s.pth
"""
import os
import sys
import time

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
APPLIO = os.path.join(ROOT, r"engine\vendor\applio")
DATASET = os.path.join(ROOT, r"models\instruments\datasets\klavier")
NAME = "tunify-klavier"
SAMPLE_RATE = 48000


def step(title, fn, **kwargs):
    start = time.time()
    print(f"== {title}", flush=True)
    result = fn(**kwargs)
    print(f"   {result} ({time.time() - start:.0f} s)", flush=True)
    if "failed" in str(result).lower():
        sys.exit(1)


def main():
    epochs = int(sys.argv[1]) if len(sys.argv) > 1 else 300
    batch = int(sys.argv[2]) if len(sys.argv) > 2 else 8
    os.chdir(APPLIO)
    sys.path.insert(0, APPLIO)
    import core

    cores = os.cpu_count() or 4
    step("Vorbereiten", core.run_preprocess_script, model_name=NAME, dataset_path=DATASET,
         sample_rate=SAMPLE_RATE, cpu_cores=cores, cut_preprocess="Automatic", process_effects=False,
         chunk_len=3.0, overlap_len=0.3, normalization_mode="none")
    step("Merkmale", core.run_extract_script, model_name=NAME, f0_method="rmvpe", cpu_cores=cores,
         gpu=0, sample_rate=SAMPLE_RATE, embedder_model="contentvec", include_mutes=2)
    step("Training", core.run_train_script, model_name=NAME, save_every_epoch=25, save_only_latest=True,
         save_every_weights=True, total_epoch=epochs, sample_rate=SAMPLE_RATE, batch_size=batch, gpu=0,
         pretrained=True, cleanup=False, index_algorithm="Auto", vocoder="HiFi-GAN")


if __name__ == "__main__":
    main()
