import platform

from . import __version__, protocol


def run() -> None:
    import torch

    gpu = None
    if torch.cuda.is_available():
        props = torch.cuda.get_device_properties(0)
        gpu = {
            "name": props.name,
            "vram_gb": round(props.total_memory / 1024**3, 1),
            "compute_capability": f"{props.major}.{props.minor}",
        }
    protocol.emit(
        "result",
        engine=__version__,
        python=platform.python_version(),
        torch=torch.__version__,
        cuda=torch.version.cuda,
        gpu=gpu,
    )
