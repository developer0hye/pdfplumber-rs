"""Type stubs for pdfplumber (pdfplumber-rs Python bindings)."""

from __future__ import annotations

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

BBox = tuple[float, float, float, float]

# ---------------------------------------------------------------------------
# Classes
# ---------------------------------------------------------------------------

class PDF:
    """A PDF document opened for extraction."""

    @staticmethod
    def open(path: str) -> PDF:
        """Open a PDF file from a filesystem path."""
        ...

    @staticmethod
    def open_bytes(data: bytes) -> PDF:
        """Open a PDF from bytes in memory."""
        ...

    @property
    def pages(self) -> list[Page]:
        """The list of pages in the PDF."""
        ...

    @property
    def metadata(self) -> MetadataDict:
        """Document metadata as a dict with keys: title, author, subject, keywords, creator, producer, creation_date, mod_date."""
        ...

    def bookmarks(self) -> list[BookmarkDict]:
        """Document bookmarks (outline / table of contents) as list[dict]."""
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
