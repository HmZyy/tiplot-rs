from pathlib import Path
from PyQt6.QtGui import QFont
from PyQt6.QtWidgets import (
    QMainWindow,
    QScrollArea,
    QTabWidget,
    QVBoxLayout,
    QWidget,
)
from PyQt6.QtCore import QSettings, Qt
from gui.modern_tab_bar import ModernTabBar
from gui.ulg_tab import ULGTab
from gui.ardupilot_tab import ArduPilotTab
from gui.mavlink_tab import MAVLinkTab
from gui.receiver_tab import ReceiverTab


class MainWindow(QMainWindow):
    def __init__(self):
        super().__init__()
        
        config_dir = Path.home() / ".config" / "tiplot"
        config_dir.mkdir(parents=True, exist_ok=True)
        
        self.settings = QSettings(
            str(config_dir / "loader.ini"),
            QSettings.Format.IniFormat
        )
        
        self.init_ui()
        self.load_settings()
    
    def init_ui(self):
        self.setWindowTitle("TiPlot Loader")
        self.setGeometry(100, 100, 1120, 820)
        self.setMinimumSize(980, 720)
        self.setFont(QFont("DejaVu Sans", 10))
        
        central_widget = QWidget()
        main_layout = QVBoxLayout()
        main_layout.setContentsMargins(24, 24, 24, 24)
        main_layout.setSpacing(18)
        
        scroll_area = QScrollArea()
        scroll_area.setWidgetResizable(True)
        scroll_area.setHorizontalScrollBarPolicy(Qt.ScrollBarPolicy.ScrollBarAsNeeded)
        scroll_area.setVerticalScrollBarPolicy(Qt.ScrollBarPolicy.ScrollBarAsNeeded)
        scroll_area.setFrameShape(QScrollArea.Shape.NoFrame)
        
        content_widget = QWidget()
        content_layout = QVBoxLayout()
        content_layout.setContentsMargins(0, 0, 0, 0)
        
        self.tabs = QTabWidget()
        self.tabs.setTabBar(ModernTabBar())
        self.tabs.setDocumentMode(True)
        self.receiver_tab = ReceiverTab(self.settings)
        self.ulg_tab = ULGTab(self.settings, self.receiver_tab)
        self.ardupilot_tab = ArduPilotTab(self.settings, self.receiver_tab)
        self.mavlink_tab = MAVLinkTab(self.settings, self.receiver_tab)
        
        self.tabs.addTab(self.ulg_tab, "ULG File")
        self.tabs.addTab(self.ardupilot_tab, "ArduPilot Log")
        self.tabs.addTab(self.mavlink_tab, "MAVLink Stream")
        self.tabs.addTab(self.receiver_tab, "Receiver")
        
        content_layout.addWidget(self.tabs)
        content_widget.setLayout(content_layout)
        
        scroll_area.setWidget(content_widget)
        
        main_layout.addWidget(scroll_area)
        central_widget.setLayout(main_layout)
        self.setCentralWidget(central_widget)
        
        self.setStyleSheet("""
            QMainWindow {
                background-color: #07111f;
            }
            QWidget {
                background-color: #07111f;
                color: #e6eef8;
                selection-background-color: #1d4ed8;
                selection-color: #f8fbff;
            }
            QScrollArea {
                border: none;
                background: transparent;
            }
            QGroupBox {
                background-color: #0d1728;
                border: 1px solid #1d2f47;
                border-radius: 16px;
                margin-top: 18px;
                padding: 18px 16px 16px 16px;
                font-weight: 600;
                font-size: 12px;
            }
            QGroupBox::title {
                subcontrol-origin: margin;
                left: 14px;
                padding: 0 8px;
                color: #dbe8f5;
            }
            QLineEdit, QSpinBox, QDoubleSpinBox, QComboBox {
                min-height: 38px;
                background-color: #0a1422;
                border: 1px solid #233750;
                border-radius: 10px;
                padding: 6px 10px;
                color: #e6eef8;
            }
            QLineEdit:focus, QSpinBox:focus, QDoubleSpinBox:focus, QComboBox:focus {
                border: 1px solid #2f81f7;
            }
            QComboBox::drop-down {
                border: none;
                width: 28px;
            }
            QComboBox::down-arrow {
                image: none;
                border-left: 5px solid transparent;
                border-right: 5px solid transparent;
                border-top: 5px solid #9eb6cf;
                margin-right: 5px;
            }
            QTextEdit {
                background-color: #09111d;
                border: 1px solid #1d2f47;
                border-radius: 16px;
                color: #d7e3f0;
                font-family: "DejaVu Sans Mono";
                font-size: 12px;
                padding: 10px;
            }
            QPushButton {
                min-height: 40px;
                background-color: #15263d;
                color: white;
                border: 1px solid #26486e;
                border-radius: 12px;
                padding: 8px 16px;
                font-weight: 600;
            }
            QPushButton:hover {
                background-color: #1c3452;
                border-color: #3d6ea6;
            }
            QTabWidget::pane {
                border: none;
                border-radius: 18px;
                background: transparent;
                margin-top: 4px;
            }
            QTabBar::base {
                border: none;
                background: transparent;
            }
            QRadioButton {
                spacing: 8px;
                color: #cbd8e6;
            }
            QRadioButton::indicator {
                width: 16px;
                height: 16px;
            }
            QRadioButton::indicator::unchecked {
                border: 1px solid #466482;
                border-radius: 8px;
                background: #0a1422;
            }
            QRadioButton::indicator::checked {
                border: 1px solid #2f81f7;
                border-radius: 8px;
                background: #2f81f7;
            }
            QScrollBar:vertical {
                background-color: #08111d;
                width: 12px;
                border-radius: 6px;
            }
            QScrollBar::handle:vertical {
                background-color: #29425f;
                border-radius: 6px;
                min-height: 20px;
            }
            QScrollBar::handle:vertical:hover {
                background-color: #3e648e;
            }
            QScrollBar::add-line:vertical, QScrollBar::sub-line:vertical {
                height: 0px;
            }
            QScrollBar:horizontal {
                background-color: #08111d;
                height: 12px;
                border-radius: 6px;
            }
            QScrollBar::handle:horizontal {
                background-color: #29425f;
                border-radius: 6px;
                min-width: 20px;
            }
            QScrollBar::handle:horizontal:hover {
                background-color: #3e648e;
            }
            QScrollBar::add-line:horizontal, QScrollBar::sub-line:horizontal {
                width: 0px;
            }
        """)
    
    def load_settings(self):
        """Load saved settings for all tabs"""
        self.receiver_tab.load_settings()
        self.ulg_tab.load_settings()
        self.ardupilot_tab.load_settings()
        self.mavlink_tab.load_settings()
    
    def save_settings(self):
        """Save current settings from all tabs"""
        self.receiver_tab.save_settings()
        self.ulg_tab.save_settings()
        self.ardupilot_tab.save_settings()
        self.mavlink_tab.save_settings()
    
    def closeEvent(self, event):
        if self.mavlink_tab.streamer:
            self.mavlink_tab.stop_streaming()
        
        self.save_settings()
        event.accept()
