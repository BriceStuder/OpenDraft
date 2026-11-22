"""Generates ezdxf_ucs.dxf, a DXF with user coordinate systems (OpenDraft's
custom coordinate systems) from an independent writer (ezdxf, AutoCAD 2018
format), for the reader tests in ezdxf_ucs.rs.

    python -m pip install ezdxf
    python make_ezdxf_ucs.py

What is in it:
- the header's current UCS: origin (10, -5), X axis at 30 degrees;
- the UCS table: "Wing A" (origin (100, 50), X axis at 90 degrees) and
  "Tilted" (its Y axis along world Z: 3D, so read as nothing);
- "Layout1": a viewport with its own UCS, origin (2, 3), X axis at 45 degrees.
"""
import math

import ezdxf

doc = ezdxf.new("R2018", setup=True)
msp = doc.modelspace()
msp.add_line((0, 0), (100, 50))


def axes(degrees):
    a = math.radians(degrees)
    return (math.cos(a), math.sin(a), 0), (-math.sin(a), math.cos(a), 0)


x, y = axes(30)
doc.header["$UCSORG"] = (10, -5, 0)
doc.header["$UCSXDIR"] = x
doc.header["$UCSYDIR"] = y

x, y = axes(90)
doc.ucs.new("Wing A", dxfattribs={"origin": (100, 50, 0), "xaxis": x, "yaxis": y})
doc.ucs.new(
    "Tilted", dxfattribs={"origin": (0, 0, 0), "xaxis": (1, 0, 0), "yaxis": (0, 0, 1)}
)

layout1 = doc.layouts.get("Layout1")
viewport = layout1.add_viewport(
    center=(100, 80), size=(50, 40), view_center_point=(10, 5), view_height=80
)
x, y = axes(45)
viewport.dxf.ucs_origin = (2, 3, 0)
viewport.dxf.ucs_x_axis = x
viewport.dxf.ucs_y_axis = y
viewport.dxf.ucs_per_viewport = 1

doc.saveas("ezdxf_ucs.dxf")
