slint::slint! {
    export component JitaWindow inherits Window {
        in-out property <string> current_state: "input";
        in-out property <string> input_text: "";
        in property <string> status_text: "";
        in property <string> script_name: "";
        in property <string> script_content: "";
        in property <string> script_description: "";
        in property <bool> asr_active: false;
        in property <bool> uv_available: true;

        // Settings properties
        in-out property <string> settings_api_key: "";
        in-out property <string> settings_api_base: "";
        in-out property <string> settings_model: "";
        in-out property <bool> settings_visible: false;

        callback submit_input(string);
        callback execute_script();
        callback cancel();
        callback toggle_asr();
        callback save_settings();
        callback open_settings();
        callback close_settings();

        title: "Jita";
        preferred-width: 600px;
        preferred-height: 400px;
        background: #ffffff;

        if !root.uv_available: Rectangle {
            height: 40px;
            background: #fff3cd;
            border-color: #ffeaa7;
            border-width: 1px;

            Text {
                x: 12px;
                y: (parent.height - self.preferred-height) / 2;
                text: "未检测到 uv，脚本执行功能不可用。请安装 uv。";
                font-size: 13px;
                color: #856404;
            }
        }

        // === SETTINGS PANEL ===
        if root.settings_visible:
        VerticalLayout {
            padding: 20px;
            spacing: 16px;

            Text {
                text: "设置";
                font-size: 22px;
                font-weight: 700;
                color: #1a1a1a;
            }

            Rectangle {
                height: 2px;
                background: #e0e0e0;
            }

            // API Key
            VerticalLayout {
                spacing: 4px;

                Text {
                    text: "API Key";
                    font-size: 14px;
                    font-weight: 600;
                    color: #333333;
                }

                Rectangle {
                    border-radius: 8px;
                    border-color: #e0e0e0;
                    border-width: 1px;
                    background: #fafafa;
                    height: 40px;

                    TextInput {
                        x: 12px;
                        y: (parent.height - self.preferred-height) / 2;
                        width: parent.width - 24px;
                        text <=> root.settings_api_key;
                        font-size: 14px;
                        color: #1a1a1a;
                        single-line: true;
                    }
                }
            }

            // API Base URL
            VerticalLayout {
                spacing: 4px;

                Text {
                    text: "API Base URL";
                    font-size: 14px;
                    font-weight: 600;
                    color: #333333;
                }

                Rectangle {
                    border-radius: 8px;
                    border-color: #e0e0e0;
                    border-width: 1px;
                    background: #fafafa;
                    height: 40px;

                    TextInput {
                        x: 12px;
                        y: (parent.height - self.preferred-height) / 2;
                        width: parent.width - 24px;
                        text <=> root.settings_api_base;
                        font-size: 14px;
                        color: #1a1a1a;
                        single-line: true;
                    }
                }
            }

            // Model
            VerticalLayout {
                spacing: 4px;

                Text {
                    text: "模型";
                    font-size: 14px;
                    font-weight: 600;
                    color: #333333;
                }

                Rectangle {
                    border-radius: 8px;
                    border-color: #e0e0e0;
                    border-width: 1px;
                    background: #fafafa;
                    height: 40px;

                    TextInput {
                        x: 12px;
                        y: (parent.height - self.preferred-height) / 2;
                        width: parent.width - 24px;
                        text <=> root.settings_model;
                        font-size: 14px;
                        color: #1a1a1a;
                        single-line: true;
                    }
                }
            }

            // Buttons
            HorizontalLayout {
                spacing: 8px;
                height: 40px;

                TouchArea {
                    width: parent.width * 0.5 - 4px;

                    Rectangle {
                        border-radius: 8px;
                        background: #0078d4;

                        Text {
                            x: (parent.width - self.preferred-width) / 2;
                            y: (parent.height - self.preferred-height) / 2;
                            text: "保存";
                            font-size: 14px;
                            font-weight: 600;
                            color: #ffffff;
                        }
                    }

                    clicked => { root.save_settings(); }
                }

                TouchArea {
                    width: parent.width * 0.5 - 4px;

                    Rectangle {
                        border-radius: 8px;
                        background: #6c757d;

                        Text {
                            x: (parent.width - self.preferred-width) / 2;
                            y: (parent.height - self.preferred-height) / 2;
                            text: "取消";
                            font-size: 14px;
                            font-weight: 600;
                            color: #ffffff;
                        }
                    }

                    clicked => { root.close_settings(); }
                }
            }
        }

        // === MAIN CONTENT ===
        if !root.settings_visible:
        VerticalLayout {
            padding: 20px;
            spacing: 12px;

            // Settings button row
            HorizontalLayout {
                height: 32px;

                TouchArea {
                    width: 60px;
                    height: 32px;

                    Rectangle {
                        border-radius: 6px;
                        background: #f0f0f0;
                        border-color: #e0e0e0;
                        border-width: 1px;

                        Text {
                            x: (parent.width - self.preferred-width) / 2;
                            y: (parent.height - self.preferred-height) / 2;
                            text: "设置";
                            font-size: 13px;
                            color: #333333;
                        }
                    }

                    clicked => { root.open_settings(); }
                }
            }

            // === INPUT STATE ===
            if root.current_state == "input" || root.current_state == "generating":
            VerticalLayout {
                spacing: 8px;

                HorizontalLayout {
                    spacing: 8px;

                    Rectangle {
                        border-radius: 8px;
                        border-color: #e0e0e0;
                        border-width: 1px;
                        background: #fafafa;
                        height: 44px;

                        TextInput {
                            x: 12px;
                            y: (parent.height - self.preferred-height) / 2;
                            width: parent.width - 24px;
                            text <=> root.input_text;
                            font-size: 16px;
                            color: #1a1a1a;
                            single-line: true;
                        }
                    }

                    TouchArea {
                        width: 44px;
                        height: 44px;

                        Rectangle {
                            border-radius: 8px;
                            background: root.asr_active ? #0078d4 : #f0f0f0;

                            Text {
                                x: (parent.width - self.preferred-width) / 2;
                                y: (parent.height - self.preferred-height) / 2;
                                text: "Mic";
                                font-size: 14px;
                                color: root.asr_active ? #ffffff : #333333;
                            }
                        }

                        clicked => { root.toggle_asr(); }
                    }
                }

                if root.input_text == "" && root.current_state == "input":
                Text {
                    text: "描述你的需求，例如：把当前目录所有 jpg 转成 png";
                    font-size: 13px;
                    color: #999999;
                }

                if root.current_state == "generating":
                Text {
                    text: "AI 生成中...";
                    font-size: 14px;
                    color: #0078d4;
                }

                if root.current_state == "input" && root.input_text != "":
                TouchArea {
                    height: 40px;

                    Rectangle {
                        border-radius: 8px;
                        background: #0078d4;

                        Text {
                            x: (parent.width - self.preferred-width) / 2;
                            y: (parent.height - self.preferred-height) / 2;
                            text: "生成脚本";
                            font-size: 14px;
                            font-weight: 600;
                            color: #ffffff;
                        }
                    }

                    clicked => { root.submit_input(root.input_text); }
                }
            }

            // === REVIEWING STATE ===
            if root.current_state == "reviewing":
            VerticalLayout {
                spacing: 12px;

                Text {
                    text: root.script_name;
                    font-size: 18px;
                    font-weight: 600;
                    color: #1a1a1a;
                }

                Text {
                    text: root.script_description;
                    font-size: 13px;
                    color: #666666;
                    wrap: word-wrap;
                }

                Rectangle {
                    border-radius: 8px;
                    background: #f6f8fa;
                    border-color: #e1e4e8;
                    border-width: 1px;
                    height: 180px;

                    Flickable {
                        width: parent.width - 16px;
                        height: parent.height - 16px;
                        x: 8px;
                        y: 8px;

                        Text {
                            text: root.script_content;
                            font-size: 13px;
                            color: #333333;
                            font-family: "Consolas, monospace";
                            wrap: word-wrap;
                        }
                    }
                }

                HorizontalLayout {
                    spacing: 8px;
                    height: 40px;

                    TouchArea {
                        width: parent.width * 0.5 - 4px;

                        Rectangle {
                            border-radius: 8px;
                            background: #28a745;

                            Text {
                                x: (parent.width - self.preferred-width) / 2;
                                y: (parent.height - self.preferred-height) / 2;
                                text: "执行";
                                font-size: 14px;
                                font-weight: 600;
                                color: #ffffff;
                            }
                        }

                        clicked => { root.execute_script(); }
                    }

                    TouchArea {
                        width: parent.width * 0.5 - 4px;

                        Rectangle {
                            border-radius: 8px;
                            background: #6c757d;

                            Text {
                                x: (parent.width - self.preferred-width) / 2;
                                y: (parent.height - self.preferred-height) / 2;
                                text: "放弃";
                                font-size: 14px;
                                font-weight: 600;
                                color: #ffffff;
                            }
                        }

                        clicked => { root.cancel(); }
                    }
                }
            }

            // === STATUS MESSAGE ===
            if root.status_text != "":
            Text {
                text: root.status_text;
                font-size: 13px;
                color: #0078d4;
            }
        }
    }
}
