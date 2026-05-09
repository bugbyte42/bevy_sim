#!/usr/bin/env python3
"""Prepare canonical world region geometry from Natural Earth-style GeoJSON.

This helper is intentionally dependency-free. It turns a manually downloaded
1:110m Admin-0 countries GeoJSON file into the compact canonical JSON consumed
by sim_data and, later, the Bevy world-map viewer.
"""

from __future__ import annotations

import argparse
import json
import re
from pathlib import Path
from typing import Any


SOURCE_REF = {
    "source_dataset": "Natural Earth 1:110m Admin-0 Countries",
    "source_row_or_page": "GeoJSON feature",
    "source_quote_or_field": "properties plus geometry.coordinates",
    "url": "https://www.naturalearthdata.com/",
}


def main() -> int:
    args = parse_args()
    regions = prepare_regions(args.input, args.precision)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(regions, indent=2) + "\n", encoding="utf-8")
    print(f"wrote {len(regions)} world regions to {args.output}")
    return 0


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Convert Natural Earth-style country GeoJSON to canonical world_regions.json."
    )
    parser.add_argument("--input", type=Path, required=True, help="Input countries GeoJSON file")
    parser.add_argument(
        "--output",
        type=Path,
        default=Path("data/canonical/v0/world_regions.json"),
        help="Output canonical JSON file",
    )
    parser.add_argument(
        "--precision",
        type=int,
        default=5,
        help="Decimal places to retain for lon/lat values",
    )
    return parser.parse_args()


def prepare_regions(path: Path, precision: int) -> list[dict[str, Any]]:
    geojson = json.loads(path.read_text(encoding="utf-8"))
    if geojson.get("type") != "FeatureCollection":
        raise ValueError("expected a GeoJSON FeatureCollection")

    regions = []
    used_ids: set[str] = set()
    for index, feature in enumerate(geojson.get("features", []), start=1):
        props = feature.get("properties") or {}
        geometry = normalize_geometry(feature.get("geometry"), precision)
        if not geometry:
            continue

        iso_a3 = choose_iso_a3(props, index)
        region_id = unique_id(f"world.{slugify(iso_a3)}", used_ids)
        display_name = str(
            props.get("NAME_LONG")
            or props.get("ADMIN")
            or props.get("NAME")
            or props.get("SOVEREIGNT")
            or iso_a3
        )
        centroid_lon, centroid_lat = centroid(geometry, precision)

        regions.append(
            {
                "id": region_id,
                "display_name": display_name,
                "iso_a3": iso_a3,
                "centroid_lon": centroid_lon,
                "centroid_lat": centroid_lat,
                "geometry": geometry,
                "tags": tags_for(props),
                "source_refs": [
                    {
                        **SOURCE_REF,
                        "source_row_or_page": f"feature {index}: {display_name}",
                    }
                ],
                "confidence": "high",
                "authored_status": "trusted",
            }
        )

    return sorted(regions, key=lambda region: region["id"])


def choose_iso_a3(props: dict[str, Any], index: int) -> str:
    for key in ("ISO_A3", "ADM0_A3", "SOV_A3", "GU_A3", "BRK_A3"):
        value = props.get(key)
        if isinstance(value, str) and re.fullmatch(r"[A-Z]{3}", value):
            return value
    fallback = props.get("NAME") or props.get("ADMIN") or f"REGION_{index:03d}"
    return slugify(str(fallback)).upper()[:12]


def normalize_geometry(geometry: dict[str, Any] | None, precision: int) -> list[Any]:
    if not geometry:
        return []

    geometry_type = geometry.get("type")
    coordinates = geometry.get("coordinates")
    if geometry_type == "Polygon":
        polygons = [coordinates]
    elif geometry_type == "MultiPolygon":
        polygons = coordinates
    else:
        return []

    normalized = []
    for polygon in polygons or []:
        rings = []
        for ring in polygon or []:
            points = []
            for point in ring or []:
                if len(point) < 2:
                    continue
                lon = round(float(point[0]), precision)
                lat = round(float(point[1]), precision)
                points.append([lon, lat])
            if points:
                rings.append(points)
        if rings:
            normalized.append(rings)
    return normalized


def centroid(geometry: list[Any], precision: int) -> tuple[float, float]:
    lon_total = 0.0
    lat_total = 0.0
    count = 0
    for polygon in geometry:
        for ring in polygon:
            for lon, lat in ring:
                lon_total += lon
                lat_total += lat
                count += 1
    if count == 0:
        return 0.0, 0.0
    return round(lon_total / count, precision), round(lat_total / count, precision)


def tags_for(props: dict[str, Any]) -> list[str]:
    tags = ["world_region", "country"]
    for prefix, key in [
        ("continent", "CONTINENT"),
        ("region_un", "REGION_UN"),
        ("subregion", "SUBREGION"),
        ("region_wb", "REGION_WB"),
    ]:
        value = props.get(key)
        if isinstance(value, str) and value.strip():
            tags.append(f"{prefix}.{slugify(value)}")
    return tags


def unique_id(base: str, used_ids: set[str]) -> str:
    candidate = base
    suffix = 2
    while candidate in used_ids:
        candidate = f"{base}_{suffix}"
        suffix += 1
    used_ids.add(candidate)
    return candidate


def slugify(value: str) -> str:
    slug = re.sub(r"[^a-zA-Z0-9]+", "_", value.strip().lower()).strip("_")
    return slug or "unknown"


if __name__ == "__main__":
    raise SystemExit(main())
