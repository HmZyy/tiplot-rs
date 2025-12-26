from pathlib import Path
from PyQt6.QtWidgets import (
    QWidget, QVBoxLayout, QHBoxLayout, QGroupBox, QLabel,
    QLineEdit, QSpinBox
)


class ReceiverTab(QWidget):
    def __init__(self, settings):
        super().__init__()
        self.settings = settings
        self.init_ui()
    
    def init_ui(self):
        layout = QVBoxLayout()
        
        receiver_group = QGroupBox("Receiver Settings")
        receiver_layout = QVBoxLayout()
        
        host_layout = QHBoxLayout()
        host_layout.addWidget(QLabel("Host:"))
        self.host_input = QLineEdit("127.0.0.1")
        host_layout.addWidget(self.host_input)
        receiver_layout.addLayout(host_layout)
        
        port_layout = QHBoxLayout()
        port_layout.addWidget(QLabel("Port:"))
        self.port_input = QSpinBox()
        self.port_input.setRange(1, 65535)
        self.port_input.setValue(9999)
        port_layout.addWidget(self.port_input)
        receiver_layout.addLayout(port_layout)
        
        receiver_group.setLayout(receiver_layout)
        layout.addWidget(receiver_group)
        
        info_label = QLabel(
            "These settings are shared across all sender tabs.\n"
            "Configure the host and port where TiPlot is listening for incoming data."
        )
        info_label.setStyleSheet("color: #94a3b8; padding: 10px;")
        info_label.setWordWrap(True)
        layout.addWidget(info_label)
        
        layout.addStretch()
        self.setLayout(layout)
    
    def get_host(self):
        return self.host_input.text()
    
    def get_port(self):
        return self.port_input.value()
    
    def load_settings(self):
        """Load saved settings"""
        self.settings.beginGroup("Receiver")
        
        host = self.settings.value("host", "127.0.0.1")
        port = int(self.settings.value("port", 9999))
        
        self.host_input.setText(host)
        self.port_input.setValue(port)
        
        self.settings.endGroup()
    
    def save_settings(self):
        """Save current settings"""
        self.settings.beginGroup("Receiver")
        
        self.settings.setValue("host", self.host_input.text())
        self.settings.setValue("port", self.port_input.value())
        
        self.settings.endGroup()
