import os
from pathlib import Path
from PyQt6.QtWidgets import (
    QWidget, QVBoxLayout, QHBoxLayout, QGroupBox, QLabel,
    QPushButton, QLineEdit, QFileDialog, QTextEdit
)
from PyQt6.QtCore import QThread

from senders.ulg import ULGSender


class ULGTab(QWidget):
    def __init__(self, settings, receiver_tab):
        super().__init__()
        self.settings = settings
        self.receiver_tab = receiver_tab
        self.ulg_file = None
        self.sender_thread = None
        self.sender = None
        self.last_directory = str(Path.home())
        self.init_ui()
    
    def init_ui(self):
        layout = QVBoxLayout()
        
        file_group = QGroupBox("ULG File")
        file_layout = QVBoxLayout()
        
        file_select_layout = QHBoxLayout()
        self.file_label = QLabel("No file selected")
        self.file_label.setStyleSheet("color: #94a3b8; padding: 8px;")
        file_select_layout.addWidget(self.file_label, 1)
        
        self.browse_btn = QPushButton("Browse...")
        self.browse_btn.clicked.connect(self.browse_file)
        file_select_layout.addWidget(self.browse_btn)
        
        file_layout.addLayout(file_select_layout)
        file_group.setLayout(file_layout)
        layout.addWidget(file_group)
        
        self.send_btn = QPushButton("Send ULG File")
        self.send_btn.setEnabled(False)
        self.send_btn.clicked.connect(self.send_file)
        self.send_btn.setStyleSheet("""
            QPushButton {
                background-color: #3b82f6;
                color: white;
                padding: 10px;
                border-radius: 5px;
                font-weight: bold;
            }
            QPushButton:hover {
                background-color: #2563eb;
            }
            QPushButton:disabled {
                background-color: #64748b;
            }
        """)
        layout.addWidget(self.send_btn)
        
        self.output_text = QTextEdit()
        self.output_text.setReadOnly(True)
        layout.addWidget(self.output_text)
        
        self.setLayout(layout)
    
    def load_settings(self):
        """Load saved settings"""
        self.settings.beginGroup("ULG")
        
        self.last_directory = self.settings.value(
            "last_directory",
            str(Path.home())
        )
        
        last_file = self.settings.value("last_file", "")
        if last_file and os.path.exists(last_file):
            self.ulg_file = last_file
            self.file_label.setText(os.path.basename(last_file))
            self.send_btn.setEnabled(True)
        
        self.settings.endGroup()
    
    def save_settings(self):
        """Save current settings"""
        self.settings.beginGroup("ULG")
        
        self.settings.setValue("last_directory", self.last_directory)
        
        if self.ulg_file:
            self.settings.setValue("last_file", self.ulg_file)
        
        self.settings.endGroup()
    
    def browse_file(self):
        file_path, _ = QFileDialog.getOpenFileName(
            self,
            "Select ULG File",
            self.last_directory,
            "ULG Files (*.ulg);;All Files (*)"
        )
        
        if file_path:
            self.ulg_file = file_path
            self.last_directory = str(Path(file_path).parent)
            self.file_label.setText(os.path.basename(file_path))
            self.send_btn.setEnabled(True)
            self.log_output(f"Selected: {file_path}")
    
    def send_file(self):
        if not self.ulg_file:
            return
        
        host = self.receiver_tab.get_host()
        port = self.receiver_tab.get_port()
        
        self.send_btn.setEnabled(False)
        self.output_text.clear()
        
        self.sender = ULGSender(self.ulg_file, host, port)
        self.sender.log_signal.connect(self.log_output)
        self.sender.finished_signal.connect(self.on_finished)
        
        self.sender_thread = QThread()
        self.sender.moveToThread(self.sender_thread)
        self.sender_thread.started.connect(self.sender.run)
        self.sender_thread.start()
    
    def on_finished(self, success, message):
        self.sender_thread.quit()
        self.sender_thread.wait()
        self.send_btn.setEnabled(True)
    
    def log_output(self, text):
        self.output_text.append(text)
        self.output_text.verticalScrollBar().setValue(
            self.output_text.verticalScrollBar().maximum()
        )
