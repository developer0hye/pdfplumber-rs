"""Separate wall and resource collectors shared by Python benchmark adapters."""

from __future__ import annotations

import resource
import sys
import time
import tracemalloc


class PythonStageMetrics:
    """Observe exactly one operation without mixing wall/resource passes."""

    def __init__(self, stage_id: str, *, timed: bool, resources: bool) -> None:
        if timed and resources:
            raise ValueError("wall and resource passes must be separate")
        self.stage_id = stage_id
        self.timed = timed
        self.resources = resources
        self._wall_started_ns: int | None = None
        self._cpu_started_ns: int | None = None

    def start(self) -> None:
        """Start immediately before the requested component operation."""

        if self.resources:
            tracemalloc.start()
            tracemalloc.reset_peak()
            self._cpu_started_ns = time.process_time_ns()
        elif self.timed:
            self._wall_started_ns = time.perf_counter_ns()

    def finish(self) -> tuple[int | None, dict[str, object] | None]:
        """Stop immediately after the operation and return one pass only."""

        if self._wall_started_ns is not None:
            return time.perf_counter_ns() - self._wall_started_ns, None
        if self._cpu_started_ns is None:
            return None, None

        cpu_time_ns = time.process_time_ns() - self._cpu_started_ns
        retained_bytes, peak_traced_bytes = tracemalloc.get_traced_memory()
        snapshot = tracemalloc.take_snapshot()
        retained_allocation_count = sum(
            statistic.count for statistic in snapshot.statistics("traceback")
        )
        tracemalloc.stop()
        peak_rss = resource.getrusage(resource.RUSAGE_SELF).ru_maxrss
        peak_rss_bytes = int(peak_rss if sys.platform == "darwin" else peak_rss * 1024)
        return None, {
            "stage_id": self.stage_id,
            "cpu": {
                "clock": "process-cpu",
                "scope": "in-adapter-stage-only",
                "time_ns": cpu_time_ns,
            },
            "peak_resident_memory": {
                "scope": "adapter-process-lifetime-high-water",
                "bytes": peak_rss_bytes,
            },
            "allocations": {
                "method": "python-tracemalloc",
                "scope": "in-adapter-stage-only",
                "retained_allocation_count": retained_allocation_count,
                "retained_bytes": retained_bytes,
                "peak_traced_bytes": peak_traced_bytes,
            },
        }
