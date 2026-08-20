"""Type stubs for the private pdfplumber-rs native extension."""

from __future__ import annotations

from collections.abc import Iterable
from typing import TextIO

__version__: str

# ---------------------------------------------------------------------------
# Exception types
# ---------------------------------------------------------------------------

class PdfParseError(RuntimeError): ...
class PdfIoError(IOError): ...
class PdfFontError(RuntimeError): ...
class PdfInterpreterError(RuntimeError): ...
class PdfResourceLimitError(RuntimeError): ...
class PdfPasswordRequired(RuntimeError): ...
class PdfInvalidPassword(ValueError): ...
class PdfminerException(Exception): ...

class PDFObjRef:
    """An unresolved metadata reference retained after permissive parsing."""

    def resolve(self) -> object: ...

class PDFStream:
    """A raw PDF stream used as a metadata value."""

    attrs: dict[str, object]
    rawdata: bytes

# ---------------------------------------------------------------------------
# Type aliases for return dicts
# ---------------------------------------------------------------------------

CharDict = dict[str, object]
WordDict = dict[str, object]
LineDict = dict[str, object]
RectDict = dict[str, object]
CurveDict = dict[str, object]
ImageDict = dict[str, object]
SearchMatchDict = dict[str, object]
BookmarkDict = dict[str, object]
MetadataDict = dict[str, object]
AnnotDict = dict[str, object]
StructElementDict = dict[str, object]
FormFieldDict = dict[str, object]
SignatureDict = dict[str, object]
ValidationIssueDict = dict[str, object]
ExtractedImageDict = dict[str, object]

BBox = tuple[float, float, float, float]

# ---------------------------------------------------------------------------
# Classes
# ---------------------------------------------------------------------------

class RustPDF:
    """Rust-native document capabilities outside the compatibility surface."""

    def bookmarks(self) -> list[BookmarkDict]: ...
    def form_fields(self) -> list[FormFieldDict]: ...
    def signatures(self) -> list[SignatureDict]: ...
    def validate(self) -> list[ValidationIssueDict]: ...
    def extract_images(self, page_index: int) -> list[ExtractedImageDict]: ...

class PDF:
    """A PDF document opened for extraction."""

    cached_properties: list[str]

    @staticmethod
    def open(path: str) -> PDF:
        """Open a PDF file from a filesystem path."""
        ...

    @staticmethod
    def open_bytes(data: bytes) -> PDF:
        """Open a PDF from bytes in memory."""
        ...

    @property
    def pages_to_parse(self) -> object | None:
        """The page-number collection supplied while opening the document."""
        ...

    @property
    def stream_is_external(self) -> bool:
        """Whether the input stream remains owned by the caller."""
        ...

    @property
    def rust(self) -> RustPDF:
        """Rust-native document capabilities under an explicit namespace."""
        ...

    @property
    def pages(self) -> list[Page]:
        """The list of pages in the PDF."""
        ...

    @property
    def metadata(self) -> MetadataDict:
        """The decoded PDF information dictionary with source key spelling."""
        ...

    def flush_cache(self, properties: list[str] | None = None) -> None:
        """Discard selected cached document properties."""
        ...

    @property
    def objects(self) -> dict[str, list[dict[str, object]]]:
        """Objects from all selected pages grouped by type."""
        ...

    @property
    def annots(self) -> list[AnnotDict]:
        """Annotation dictionaries from all selected pages."""
        ...

    @property
    def hyperlinks(self) -> list[AnnotDict]:
        """URI annotation dictionaries from all selected pages."""
        ...

    @property
    def structure_tree(self) -> list[StructElementDict]:
        """Compact document structure-tree dictionaries."""
        ...

    def to_dict(self, object_types: Iterable[str] | None = None) -> dict[str, object]:
        """Return document metadata and selected page dictionaries."""
        ...

    def to_json(
        self,
        stream: TextIO | None = None,
        object_types: Iterable[str] | None = None,
        include_attrs: list[str] | None = None,
        exclude_attrs: list[str] | None = None,
        precision: int | None = None,
        indent: int | None = None,
    ) -> str | None:
        """Serialize document metadata and selected pages as JSON."""
        ...

    def to_csv(
        self,
        stream: TextIO | None = None,
        object_types: Iterable[str] | None = None,
        precision: int | None = None,
        include_attrs: list[str] | None = None,
        exclude_attrs: list[str] | None = None,
    ) -> str | None:
        """Serialize selected page objects as CSV."""
        ...

class Page:
    """A single page from a PDF document."""

    @property
    def page_number(self) -> int:
        """The 0-based page index."""
        ...

    @property
    def width(self) -> float:
        """Page width in points."""
        ...

    @property
    def height(self) -> float:
        """Page height in points."""
        ...

    def to_dict(self, object_types: Iterable[str] | None = None) -> dict[str, object]:
        """Return page geometry and requested object dictionaries."""
        ...

    def to_json(
        self,
        stream: TextIO | None = None,
        object_types: Iterable[str] | None = None,
        include_attrs: list[str] | None = None,
        exclude_attrs: list[str] | None = None,
        precision: int | None = None,
        indent: int | None = None,
    ) -> str | None:
        """Serialize page geometry and requested objects as JSON."""
        ...

    def to_csv(
        self,
        stream: TextIO | None = None,
        object_types: Iterable[str] | None = None,
        precision: int | None = None,
        include_attrs: list[str] | None = None,
        exclude_attrs: list[str] | None = None,
    ) -> str | None:
        """Serialize page objects as CSV."""
        ...

    def chars(self) -> list[CharDict]:
        """Characters on this page as list[dict]."""
        ...

    def extract_text(self, layout: bool = False) -> str:
        """Extract text from this page."""
        ...

    def extract_words(
        self,
        x_tolerance: float = 3.0,
        y_tolerance: float = 3.0,
    ) -> list[WordDict]:
        """Extract words from this page."""
        ...

    def find_tables(self) -> list[Table]:
        """Find tables on this page."""
        ...

    def extract_tables(self) -> list[list[list[str | None]]]:
        """Extract table content as list[list[list[str|None]]]."""
        ...

    def lines(self) -> list[LineDict]:
        """Lines on this page as list[dict]."""
        ...

    def rects(self) -> list[RectDict]:
        """Rectangles on this page as list[dict]."""
        ...

    def curves(self) -> list[CurveDict]:
        """Curves on this page as list[dict]."""
        ...

    def images(self) -> list[ImageDict]:
        """Images on this page as list[dict]."""
        ...

    @property
    def annots(self) -> list[AnnotDict]:
        """Annotation dictionaries on this page."""
        ...

    @property
    def hyperlinks(self) -> list[AnnotDict]:
        """Annotation dictionaries whose URI is not null."""
        ...

    @property
    def structure_tree(self) -> list[StructElementDict]:
        """Compact structure-tree dictionaries for this page."""
        ...

    def crop(self, bbox: BBox) -> CroppedPage:
        """Crop this page to a bounding box (x0, top, x1, bottom)."""
        ...

    def within_bbox(self, bbox: BBox) -> CroppedPage:
        """Filter to objects fully within the given bbox."""
        ...

    def outside_bbox(self, bbox: BBox) -> CroppedPage:
        """Filter to objects outside the given bbox."""
        ...

    def search(
        self,
        pattern: str,
        regex: bool = True,
        case: bool = True,
    ) -> list[SearchMatchDict]:
        """Search for a text pattern on this page."""
        ...

class Table:
    """A detected table from a PDF page."""

    @property
    def bbox(self) -> BBox:
        """Bounding box as (x0, top, x1, bottom)."""
        ...

    @property
    def rows(self) -> list[list[dict[str, object]]]:
        """Cells organized into rows as list[list[dict]]."""
        ...

    @property
    def accuracy(self) -> float:
        """Percentage of non-empty cells (0.0 to 1.0)."""
        ...

    def extract(self) -> list[list[str | None]]:
        """Extract table content as list of rows, each row a list of cell text values."""
        ...

class CroppedPage:
    """A spatially filtered view of a PDF page."""

    @property
    def width(self) -> float:
        """Width of the cropped region."""
        ...

    @property
    def height(self) -> float:
        """Height of the cropped region."""
        ...

    def chars(self) -> list[CharDict]:
        """Characters in the cropped region as list[dict]."""
        ...

    def extract_text(self, layout: bool = False) -> str:
        """Extract text from the cropped region."""
        ...

    def extract_words(
        self,
        x_tolerance: float = 3.0,
        y_tolerance: float = 3.0,
    ) -> list[WordDict]:
        """Extract words from the cropped region."""
        ...

    def find_tables(self) -> list[Table]:
        """Find tables in the cropped region."""
        ...

    def extract_tables(self) -> list[list[list[str | None]]]:
        """Extract table content from the cropped region."""
        ...

    def lines(self) -> list[LineDict]:
        """Lines in the cropped region as list[dict]."""
        ...

    def rects(self) -> list[RectDict]:
        """Rects in the cropped region as list[dict]."""
        ...

    def curves(self) -> list[CurveDict]:
        """Curves in the cropped region as list[dict]."""
        ...

    def images(self) -> list[ImageDict]:
        """Images in the cropped region as list[dict]."""
        ...

    def crop(self, bbox: BBox) -> CroppedPage:
        """Further crop this cropped page."""
        ...

    def within_bbox(self, bbox: BBox) -> CroppedPage:
        """Filter to objects fully within the given bbox."""
        ...

    def outside_bbox(self, bbox: BBox) -> CroppedPage:
        """Filter to objects outside the given bbox."""
        ...
