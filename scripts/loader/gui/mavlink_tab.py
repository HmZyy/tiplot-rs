import os
from pathlib import Path
from PyQt6.QtWidgets import (
    QWidget, QVBoxLayout, QHBoxLayout, QGroupBox, QLabel,
    QPushButton, QLineEdit, QFileDialog, QSpinBox, QTextEdit,
    QRadioButton, QButtonGroup, QDoubleSpinBox, QMessageBox, QComboBox
)
from PyQt6.QtCore import QThread

from senders.mavlink import MAVLinkStreamer


def get_serial_ports():
    """Get list of available serial ports from /dev/serial/by-id/"""
    ports = []
    by_id_path = Path("/dev/serial/by-id")
    
    if by_id_path.exists():
        for device in sorted(by_id_path.iterdir()):
            if device.is_symlink():
                real_path = device.resolve()
                ports.append({
                    'path': str(real_path),
                    'name': device.name,
                    'display': f"{device.name} ({real_path.name})"
                })
    
    if not ports:
        for device_pattern in ["/dev/ttyACM*", "/dev/ttyUSB*"]:
            for device in sorted(Path("/dev").glob(device_pattern[5:])):
                ports.append({
                    'path': str(device),
                    'name': device.name,
                    'display': device.name
                })
    
    return ports


class MAVLinkTab(QWidget):
    def __init__(self, settings, receiver_tab):
        super().__init__()
        self.settings = settings
        self.receiver_tab = receiver_tab
        self.streamer = None
        self.streamer_thread = None
        self.mavlink_file = None
        self.last_directory = str(Path.home())
        self.init_ui()
    
    def init_ui(self):
        layout = QVBoxLayout()
        
        mode_group = QGroupBox("Connection Mode")
        mode_layout = QVBoxLayout()
        
        self.mode_group = QButtonGroup()
        self.serial_radio = QRadioButton("Serial")
        self.tcp_radio = QRadioButton("TCP")
        self.udp_radio = QRadioButton("UDP")
        self.file_radio = QRadioButton("File")
        
        self.serial_radio.setChecked(True)
        
        self.mode_group.addButton(self.serial_radio, 0)
        self.mode_group.addButton(self.tcp_radio, 1)
        self.mode_group.addButton(self.udp_radio, 2)
        self.mode_group.addButton(self.file_radio, 3)
        
        mode_layout.addWidget(self.serial_radio)
        mode_layout.addWidget(self.tcp_radio)
        mode_layout.addWidget(self.udp_radio)
        mode_layout.addWidget(self.file_radio)
        
        self.mode_group.buttonClicked.connect(self.on_mode_changed)
        
        mode_group.setLayout(mode_layout)
        layout.addWidget(mode_group)
        
        params_group = QGroupBox("Connection Parameters")
        params_layout = QVBoxLayout()
        
        self.serial_widget = QWidget()
        serial_layout = QVBoxLayout()
        
        device_layout = QHBoxLayout()
        device_layout.addWidget(QLabel("Device:"))
        self.device_combo = QComboBox()
        self.device_combo.setEditable(True)
        device_layout.addWidget(self.device_combo, 1)
        
        self.refresh_serial_btn = QPushButton("🔄")
        self.refresh_serial_btn.setFixedWidth(40)
        self.refresh_serial_btn.setToolTip("Refresh serial ports")
        self.refresh_serial_btn.clicked.connect(self.refresh_serial_ports)
        device_layout.addWidget(self.refresh_serial_btn)
        
        serial_layout.addLayout(device_layout)
        
        baud_layout = QHBoxLayout()
        baud_layout.addWidget(QLabel("Baudrate:"))
        self.baud_input = QSpinBox()
        self.baud_input.setRange(9600, 921600)
        self.baud_input.setValue(115200)
        baud_layout.addWidget(self.baud_input)
        serial_layout.addLayout(baud_layout)
        
        self.serial_widget.setLayout(serial_layout)
        params_layout.addWidget(self.serial_widget)
        
        self.tcp_widget = QWidget()
        tcp_layout = QVBoxLayout()
        
        tcp_addr_layout = QHBoxLayout()
        tcp_addr_layout.addWidget(QLabel("Address:"))
        self.tcp_addr_input = QLineEdit("0.0.0.0")
        tcp_addr_layout.addWidget(self.tcp_addr_input)
        tcp_layout.addLayout(tcp_addr_layout)
        
        tcp_port_layout = QHBoxLayout()
        tcp_port_layout.addWidget(QLabel("Port:"))
        self.tcp_port_input = QSpinBox()
        self.tcp_port_input.setRange(1, 65535)
        self.tcp_port_input.setValue(5760)
        tcp_port_layout.addWidget(self.tcp_port_input)
        tcp_layout.addLayout(tcp_port_layout)
        
        self.tcp_widget.setLayout(tcp_layout)
        self.tcp_widget.hide()
        params_layout.addWidget(self.tcp_widget)
        
        self.udp_widget = QWidget()
        udp_layout = QVBoxLayout()
        
        udp_addr_layout = QHBoxLayout()
        udp_addr_layout.addWidget(QLabel("Address:"))
        self.udp_addr_input = QLineEdit("127.0.0.1")
        udp_addr_layout.addWidget(self.udp_addr_input)
        udp_layout.addLayout(udp_addr_layout)
        
        udp_port_layout = QHBoxLayout()
        udp_port_layout.addWidget(QLabel("Port:"))
        self.udp_port_input = QSpinBox()
        self.udp_port_input.setRange(1, 65535)
        self.udp_port_input.setValue(14550)
        udp_port_layout.addWidget(self.udp_port_input)
        udp_layout.addLayout(udp_port_layout)
        
        self.udp_widget.setLayout(udp_layout)
        self.udp_widget.hide()
        params_layout.addWidget(self.udp_widget)
        
        self.file_widget = QWidget()
        file_layout = QVBoxLayout()
        
        file_select_layout = QHBoxLayout()
        self.file_label = QLabel("No file selected")
        self.file_label.setStyleSheet("color: #94a3b8;")
        file_select_layout.addWidget(self.file_label, 1)
        
        self.file_browse_btn = QPushButton("Browse...")
        self.file_browse_btn.clicked.connect(self.browse_mavlink_file)
        file_select_layout.addWidget(self.file_browse_btn)
        file_layout.addLayout(file_select_layout)
        
        self.file_widget.setLayout(file_layout)
        self.file_widget.hide()
        params_layout.addWidget(self.file_widget)
        
        params_group.setLayout(params_layout)
        layout.addWidget(params_group)
        
        stream_group = QGroupBox("Streaming Settings")
        stream_layout = QVBoxLayout()
        
        rate_layout = QHBoxLayout()
        rate_layout.addWidget(QLabel("Update Rate (Hz):"))
        self.rate_input = QDoubleSpinBox()
        self.rate_input.setRange(0.1, 100.0)
        self.rate_input.setValue(10.0)
        self.rate_input.setSingleStep(0.5)
        rate_layout.addWidget(self.rate_input)
        stream_layout.addLayout(rate_layout)
        
        stream_group.setLayout(stream_layout)
        layout.addWidget(stream_group)
        
        button_layout = QHBoxLayout()
        
        self.start_btn = QPushButton("Start Streaming")
        self.start_btn.clicked.connect(self.start_streaming)
        self.start_btn.setStyleSheet("""
            QPushButton {
                background-color: #10b981;
                color: white;
                padding: 10px;
                border-radius: 5px;
                font-weight: bold;
            }
            QPushButton:hover {
                background-color: #059669;
            }
            QPushButton:disabled {
                background-color: #64748b;
            }
        """)
        button_layout.addWidget(self.start_btn)
        
        self.stop_btn = QPushButton("Stop Streaming")
        self.stop_btn.clicked.connect(self.stop_streaming)
        self.stop_btn.setEnabled(False)
        self.stop_btn.setStyleSheet("""
            QPushButton {
                background-color: #ef4444;
                color: white;
                padding: 10px;
                border-radius: 5px;
                font-weight: bold;
            }
            QPushButton:hover {
                background-color: #dc2626;
            }
            QPushButton:disabled {
                background-color: #64748b;
            }
        """)
        button_layout.addWidget(self.stop_btn)
        
        layout.addLayout(button_layout)
        
        self.output_text = QTextEdit()
        self.output_text.setReadOnly(True)
        layout.addWidget(self.output_text)
        
        self.setLayout(layout)
        
        # Initialize serial ports
        self.refresh_serial_ports()
    
    def refresh_serial_ports(self):
        """Refresh the list of available serial ports"""
        current_text = self.device_combo.currentText()
        self.device_combo.clear()
        
        ports = get_serial_ports()
        
        if ports:
            for port in ports:
                self.device_combo.addItem(port['display'], port['path'])
            
            # Try to restore previous selection
            index = self.device_combo.findData(current_text)
            if index >= 0:
                self.device_combo.setCurrentIndex(index)
            elif current_text:
                # If not found in dropdown, keep it as editable text
                self.device_combo.setEditText(current_text)
        else:
            self.device_combo.addItem("/dev/ttyACM0")
    
    def load_settings(self):
        """Load saved settings"""
        self.settings.beginGroup("MAVLink")
        
        # Load connection mode
        mode = int(self.settings.value("mode", 0))
        if mode == 0:
            self.serial_radio.setChecked(True)
        elif mode == 1:
            self.tcp_radio.setChecked(True)
        elif mode == 2:
            self.udp_radio.setChecked(True)
        elif mode == 3:
            self.file_radio.setChecked(True)
        
        # Load serial settings
        serial_device = self.settings.value("serial_device", "/dev/ttyACM0")
        self.device_combo.setEditText(serial_device)
        self.baud_input.setValue(int(self.settings.value("baudrate", 115200)))
        
        # Load TCP settings
        self.tcp_addr_input.setText(self.settings.value("tcp_address", "0.0.0.0"))
        self.tcp_port_input.setValue(int(self.settings.value("tcp_port", 5760)))
        
        # Load UDP settings
        self.udp_addr_input.setText(self.settings.value("udp_address", "127.0.0.1"))
        self.udp_port_input.setValue(int(self.settings.value("udp_port", 14550)))
        
        # Load file settings
        self.last_directory = self.settings.value(
            "last_directory",
            str(Path.home())
        )
        last_file = self.settings.value("last_file", "")
        if last_file and os.path.exists(last_file):
            self.mavlink_file = last_file
            self.file_label.setText(os.path.basename(last_file))
        
        # Load streaming settings
        self.rate_input.setValue(float(self.settings.value("rate", 10.0)))
        
        self.settings.endGroup()
        
        # Update UI based on loaded mode
        self.on_mode_changed()
    
    def save_settings(self):
        """Save current settings"""
        self.settings.beginGroup("MAVLink")
        
        # Save connection mode
        self.settings.setValue("mode", self.mode_group.checkedId())
        
        # Save serial settings
        self.settings.setValue("serial_device", self.device_combo.currentText())
        self.settings.setValue("baudrate", self.baud_input.value())
        
        # Save TCP settings
        self.settings.setValue("tcp_address", self.tcp_addr_input.text())
        self.settings.setValue("tcp_port", self.tcp_port_input.value())
        
        # Save UDP settings
        self.settings.setValue("udp_address", self.udp_addr_input.text())
        self.settings.setValue("udp_port", self.udp_port_input.value())
        
        # Save file settings
        self.settings.setValue("last_directory", self.last_directory)
        if self.mavlink_file:
            self.settings.setValue("last_file", self.mavlink_file)
        
        # Save streaming settings
        self.settings.setValue("rate", self.rate_input.value())
        
        self.settings.endGroup()
    
    def on_mode_changed(self):
        mode_id = self.mode_group.checkedId()
        
        self.serial_widget.setVisible(mode_id == 0)
        self.tcp_widget.setVisible(mode_id == 1)
        self.udp_widget.setVisible(mode_id == 2)
        self.file_widget.setVisible(mode_id == 3)
    
    def browse_mavlink_file(self):
        file_path, _ = QFileDialog.getOpenFileName(
            self,
            "Select MAVLink Log File",
            self.last_directory,
            "All Files (*)"
        )
        
        if file_path:
            self.file_label.setText(os.path.basename(file_path))
            self.mavlink_file = file_path
            self.last_directory = str(Path(file_path).parent)
    
    def start_streaming(self):
        mode_id = self.mode_group.checkedId()
        
        connection_str = None
        baudrate = None
        
        if mode_id == 0:
            # Use the actual path from combo box data if available
            current_index = self.device_combo.currentIndex()
            if current_index >= 0:
                connection_str = self.device_combo.itemData(current_index)
                if not connection_str:
                    connection_str = self.device_combo.currentText()
            else:
                connection_str = self.device_combo.currentText()
            baudrate = self.baud_input.value()
        elif mode_id == 1:
            connection_str = f"tcp:{self.tcp_addr_input.text()}:{self.tcp_port_input.value()}"
        elif mode_id == 2:
            connection_str = f"udp:{self.udp_addr_input.text()}:{self.udp_port_input.value()}"
        elif mode_id == 3:
            if not self.mavlink_file:
                QMessageBox.warning(self, "Error", "Please select a MAVLink log file")
                return
            connection_str = self.mavlink_file
        
        host = self.receiver_tab.get_host()
        port = self.receiver_tab.get_port()
        rate = self.rate_input.value()
        
        self.output_text.clear()
        
        self.streamer = MAVLinkStreamer(connection_str, baudrate, host, port, rate)
        self.streamer.log_signal.connect(self.log_output)
        self.streamer.status_signal.connect(self.update_status)
        
        self.streamer_thread = QThread()
        self.streamer.moveToThread(self.streamer_thread)
        self.streamer_thread.started.connect(self.streamer.run)
        self.streamer_thread.start()
        
        self.start_btn.setEnabled(False)
        self.stop_btn.setEnabled(True)
    
    def stop_streaming(self):
        if self.streamer:
            self.log_output("\nStopping stream...")
            self.streamer.stop()
        
        if self.streamer_thread:
            self.streamer_thread.quit()
            self.streamer_thread.wait()
        
        self.start_btn.setEnabled(True)
        self.stop_btn.setEnabled(False)
    
    def log_output(self, text):
        self.output_text.append(text)
        self.output_text.verticalScrollBar().setValue(
            self.output_text.verticalScrollBar().maximum()
        )
    
    def update_status(self, text):
        cursor = self.output_text.textCursor()
        cursor.movePosition(cursor.MoveOperation.End)
        cursor.movePosition(cursor.MoveOperation.StartOfLine, cursor.MoveMode.KeepAnchor)
        
        selected = cursor.selectedText()
        if selected and selected[0].isdigit():
            cursor.removeSelectedText()
            cursor.deletePreviousChar()
        
        cursor.movePosition(cursor.MoveOperation.End)
        self.output_text.setTextCursor(cursor)
        self.output_text.insertPlainText(text + '\n')
