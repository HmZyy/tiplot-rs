import os
from pathlib import Path
from PyQt6.QtWidgets import QMainWindow, QWidget, QVBoxLayout, QTabWidget, QScrollArea
from PyQt6.QtCore import QSettings, Qt
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
        self.setGeometry(100, 100, 800, 700)
        
        central_widget = QWidget()
        main_layout = QVBoxLayout()
        main_layout.setContentsMargins(0, 0, 0, 0)
        
        scroll_area = QScrollArea()
        scroll_area.setWidgetResizable(True)
        scroll_area.setHorizontalScrollBarPolicy(Qt.ScrollBarPolicy.ScrollBarAsNeeded)
        scroll_area.setVerticalScrollBarPolicy(Qt.ScrollBarPolicy.ScrollBarAsNeeded)
        
        content_widget = QWidget()
        content_layout = QVBoxLayout()
        
        self.tabs = QTabWidget()
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
                background-color: #1e293b;
            }
            QWidget {
                background-color: #1e293b;
                color: #e2e8f0;
            }
            QScrollArea {
                border: none;
                background-color: #1e293b;
            }
            QGroupBox {
                border: 1px solid #475569;
                border-radius: 5px;
                margin-top: 10px;
                padding-top: 10px;
                font-weight: bold;
            }
            QGroupBox::title {
                subcontrol-origin: margin;
                left: 10px;
                padding: 0 5px;
            }
            QLineEdit, QSpinBox, QDoubleSpinBox, QComboBox {
                background-color: #334155;
                border: 1px solid #475569;
                border-radius: 3px;
                padding: 5px;
                color: #e2e8f0;
            }
            QComboBox::drop-down {
                border: none;
            }
            QComboBox::down-arrow {
                image: none;
                border-left: 5px solid transparent;
                border-right: 5px solid transparent;
                border-top: 5px solid #e2e8f0;
                margin-right: 5px;
            }
            QTextEdit {
                background-color: #0f172a;
                border: 1px solid #475569;
                border-radius: 3px;
                color: #e2e8f0;
                font-family: monospace;
            }
            QPushButton {
                background-color: #475569;
                color: white;
                border: none;
                border-radius: 3px;
                padding: 8px 16px;
            }
            QPushButton:hover {
                background-color: #64748b;
            }
            QTabWidget::pane {
                border: 1px solid #475569;
                background-color: #1e293b;
            }
            QTabBar::tab {
                background-color: #334155;
                color: #94a3b8;
                padding: 10px 20px;
                border-top-left-radius: 5px;
                border-top-right-radius: 5px;
            }
            QTabBar::tab:selected {
                background-color: #1e293b;
                color: #3b82f6;
                border-bottom: 2px solid #3b82f6;
            }
            QRadioButton {
                spacing: 5px;
            }
            QRadioButton::indicator {
                width: 15px;
                height: 15px;
            }
            QScrollBar:vertical {
                background-color: #1e293b;
                width: 12px;
                border-radius: 6px;
            }
            QScrollBar::handle:vertical {
                background-color: #475569;
                border-radius: 6px;
                min-height: 20px;
            }
            QScrollBar::handle:vertical:hover {
                background-color: #64748b;
            }
            QScrollBar::add-line:vertical, QScrollBar::sub-line:vertical {
                height: 0px;
            }
            QScrollBar:horizontal {
                background-color: #1e293b;
                height: 12px;
                border-radius: 6px;
            }
            QScrollBar::handle:horizontal {
                background-color: #475569;
                border-radius: 6px;
                min-width: 20px;
            }
            QScrollBar::handle:horizontal:hover {
                background-color: #64748b;
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
