from PyQt6.QtCore import QSize, Qt
from PyQt6.QtGui import QColor, QFont, QPainter, QPen
from PyQt6.QtWidgets import QStyle, QStyleOptionTab, QTabBar


class ModernTabBar(QTabBar):
    def __init__(self, parent=None):
        super().__init__(parent)
        self.setDrawBase(False)
        self.setExpanding(False)
        self.setElideMode(Qt.TextElideMode.ElideRight)
        self.setDocumentMode(True)
        self.setUsesScrollButtons(False)

        self._selected_bg = QColor("#0a1322")
        self._idle_bg = QColor("#0d1728")
        self._selected_border = QColor("#29598d")
        self._idle_border = QColor("#1d2f47")
        self._selected_text = QColor("#f8fbff")
        self._idle_text = QColor("#8da5bd")

        font = QFont(self.font())
        font.setWeight(QFont.Weight.DemiBold)
        self.setFont(font)

    def tabSizeHint(self, index):
        size = super().tabSizeHint(index)
        return QSize(size.width() + 22, max(size.height(), 40))

    def paintEvent(self, event):
        painter = QPainter(self)
        painter.setRenderHint(QPainter.RenderHint.Antialiasing, True)
        painter.fillRect(event.rect(), Qt.GlobalColor.transparent)

        for index in range(self.count()):
            option = QStyleOptionTab()
            self.initStyleOption(option, index)

            rect = self.tabRect(index).adjusted(0, 6, -8, 0)
            selected = index == self.currentIndex()

            background = self._selected_bg if selected else self._idle_bg
            border = self._selected_border if selected else self._idle_border
            text_color = self._selected_text if selected else self._idle_text

            painter.setPen(QPen(border, 1))
            painter.setBrush(background)
            painter.drawRoundedRect(rect, 12, 12)

            text_rect = rect.adjusted(14, 0, -14, 0)
            painter.setPen(text_color)
            painter.drawText(
                text_rect,
                Qt.AlignmentFlag.AlignCenter,
                self.style().itemTextRect(
                    painter.fontMetrics(),
                    text_rect,
                    int(Qt.AlignmentFlag.AlignCenter),
                    True,
                    option.text,
                ).isValid() and option.text or option.text,
            )

    def minimumTabSizeHint(self, index):
        return self.tabSizeHint(index)
