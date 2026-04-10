from PyQt6.QtCore import Qt, QRectF
from PyQt6.QtGui import QColor, QPainter, QPen
from PyQt6.QtWidgets import QPushButton


class ProgressButton(QPushButton):
    def __init__(
        self,
        text,
        base_color,
        hover_color,
        idle_color="#1e293b",
        disabled_color="#64748b",
        border_color="#ffffff",
        parent=None,
    ):
        super().__init__(text, parent)
        self.base_color = QColor(base_color)
        self.hover_color = QColor(hover_color)
        self.idle_color = QColor(idle_color)
        self.disabled_color = QColor(disabled_color)
        self.border_color = QColor(border_color)
        self.progress = None
        self.default_text = text
        self.setCursor(Qt.CursorShape.PointingHandCursor)
        self.setMinimumHeight(42)

    def set_progress(self, percent):
        self.progress = None if percent is None else max(0, min(100, int(percent)))
        self.update()

    def paintEvent(self, event):
        painter = QPainter(self)
        painter.setRenderHint(QPainter.RenderHint.Antialiasing, True)

        rect = self.rect().adjusted(0, 0, -1, -1)
        radius = 5.0

        if self.progress is None:
            background = self.disabled_color if not self.isEnabled() else self.base_color
            painter.setPen(Qt.PenStyle.NoPen)
            painter.setBrush(background)
            painter.drawRoundedRect(QRectF(rect), radius, radius)
        else:
            painter.setPen(Qt.PenStyle.NoPen)
            painter.setBrush(self.idle_color)
            painter.drawRoundedRect(QRectF(rect), radius, radius)

            if self.progress > 0:
                fill_width = rect.width() * self.progress / 100.0
                fill_rect = QRectF(rect.x(), rect.y(), fill_width, rect.height())
                painter.save()
                path = self._rounded_path(rect, radius)
                painter.setClipPath(path)
                painter.setBrush(self.base_color)
                painter.drawRect(fill_rect)
                painter.restore()

        border_pen = QPen(self.border_color, 1)
        painter.setPen(border_pen)
        painter.setBrush(Qt.BrushStyle.NoBrush)
        painter.drawRoundedRect(QRectF(rect), radius, radius)

        text_color = QColor("#ffffff")
        painter.setPen(text_color)
        painter.drawText(rect, Qt.AlignmentFlag.AlignCenter, self._display_text())

    def enterEvent(self, event):
        if self.progress is None and self.isEnabled():
            self.base_color, self.hover_color = self.hover_color, self.base_color
            self.update()
        super().enterEvent(event)

    def leaveEvent(self, event):
        if self.progress is None and self.isEnabled():
            self.base_color, self.hover_color = self.hover_color, self.base_color
            self.update()
        super().leaveEvent(event)

    def _rounded_path(self, rect, radius):
        from PyQt6.QtGui import QPainterPath

        path = QPainterPath()
        path.addRoundedRect(QRectF(rect), radius, radius)
        return path

    def _display_text(self):
        if self.progress is None:
            return self.default_text
        return f"{self.default_text} ({self.progress}%)"
