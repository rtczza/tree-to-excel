use anyhow::{Context, Result};
use clap::{Arg, Command};
use rust_xlsxwriter::{Format, Workbook, Worksheet};
use std::fs;
use std::io::{self, Read};

/// 文件/目录项
#[derive(Debug, Clone)]
struct TreeItem {
    name: String,
    level: usize,
    is_file: bool,
    full_path: String,
}

/// Excel行数据  
#[derive(Debug)]
struct ExcelRow {
    levels: Vec<String>,     // 每个层级的名称，如["src", "bin", "file.rs"]
    full_path: String,       // 完整路径
    max_level: usize,        // 最大层级深度
    is_file: bool,
}

/// Tree输出解析器
struct TreeParser;

impl TreeParser {
    fn new() -> Self {
        Self
    }

    /// 解析tree输出，返回扁平化的项目列表
    fn parse(&self, input: &str, include_hidden: bool) -> Result<Vec<TreeItem>> {
        let lines: Vec<&str> = input.lines().collect();
        let mut items = Vec::new();
        let mut path_stack: Vec<String> = Vec::new();
        let mut stats_line = None;
        let mut hidden_levels: Vec<usize> = Vec::new(); // 记录被过滤的隐藏目录的层级

        for line in lines {
            if line.trim().is_empty() {
                continue;
            }

            // 检查统计行
            if line.contains("directories") && line.contains("files") {
                stats_line = Some(line.trim().to_string());
                continue;
            }

            // 解析层级和名称
            if let Some((level, name)) = self.parse_line(line) {
                // 清理过期的隐藏层级记录（当前层级小于等于隐藏层级时）
                hidden_levels.retain(|&hidden_level| hidden_level < level);
                
                // 检查是否在隐藏目录内
                let in_hidden_dir = !hidden_levels.is_empty();
                
                // 过滤隐藏目录/文件（以.开头的项目，如.git）
                if !include_hidden && (name.starts_with('.') || in_hidden_dir) {
                    if name.starts_with('.') {
                        // 记录这个隐藏目录的层级，用于过滤其子项目
                        hidden_levels.push(level);
                    }
                    continue;
                }
                
                // 调整路径栈到当前层级
                path_stack.truncate(level.saturating_sub(1));
                
                // 构建完整路径
                let full_path = if path_stack.is_empty() {
                    name.clone()
                } else {
                    format!("{}/{}", path_stack.join("/"), name)
                };

                // 添加到路径栈
                path_stack.push(name.clone());

                // 判断是否为文件
                let is_file = self.is_file(&name);

                items.push(TreeItem {
                    name: name.clone(),
                    level,
                    is_file,
                    full_path,
                });
            }
        }

        // 重新计算统计信息（基于实际解析的内容）
        let file_count = items.iter().filter(|item| item.is_file).count();
        let dir_count = items.iter().filter(|item| !item.is_file).count();
        
        let stats_text = if include_hidden {
            // 如果包含隐藏目录，使用原始统计信息（如果有的话）
            stats_line.unwrap_or_else(|| format!("{} directories, {} files", dir_count, file_count))
        } else {
            // 如果过滤了隐藏目录，使用重新计算的统计信息
            format!("{} directories, {} files", dir_count, file_count)
        };
        
        items.push(TreeItem {
            name: format!("📊 统计: {}", stats_text),
            level: 0,
            is_file: false,
            full_path: format!("📊 统计: {}", stats_text),
        });



        Ok(items)
    }

    /// 解析单行，返回(层级, 名称)
    fn parse_line(&self, line: &str) -> Option<(usize, String)> {
        // 跳过根目录标记（可能是 "." 或项目名如 "utzip-0.9.0/"）
        let trimmed = line.trim();
        if trimmed == "." || (trimmed.ends_with('/') && !trimmed.contains("├") && !trimmed.contains("└")) {
            return None;
        }

        // 清理行，移除ANSI转义序列
        let clean_line = self.remove_ansi_codes(line);
        let chars: Vec<char> = clean_line.chars().collect();
        let mut pos = 0;
        let mut level = 0;

        // 计算层级：支持两种缩进模式
        // 1. "│   " 模式（垂直线 + 3个空格）
        // 2. "    " 模式（4个空格，用于最后的子目录）
        // 注意：tree输出可能使用不同类型的空格字符(U+0020普通空格, U+00A0非断空格)
        while pos + 3 < chars.len() {
            if chars[pos] == '│' && 
               chars[pos + 1].is_whitespace() && 
               chars[pos + 2].is_whitespace() && 
               chars[pos + 3].is_whitespace() {
                level += 1;
                pos += 4;
            } else if chars[pos] == ' ' && 
                      chars[pos + 1] == ' ' && 
                      chars[pos + 2] == ' ' && 
                      chars[pos + 3] == ' ' {
                // 支持纯空格缩进（4个空格）
                level += 1;
                pos += 4;
            } else {
                break;
            }
        }

        // 查找并跳过tree连接符 "├──" 或 "└──"
        if pos + 2 < chars.len() && 
           (chars[pos] == '├' || chars[pos] == '└') &&
           chars[pos + 1] == '─' && 
           chars[pos + 2] == '─' {
            pos += 3;
            // 跳过可能的空格
            if pos < chars.len() && chars[pos] == ' ' {
                pos += 1;
            }
        } else {
            // 没有找到标准的tree符号，可能不是有效的tree行
            return None;
        }

        // 提取剩余部分作为文件/目录名
        if pos >= chars.len() {
            return None;
        }

        let name: String = chars[pos..].iter().collect::<String>().trim().to_string();
        
        if name.is_empty() {
            None
        } else {
            Some((level + 1, name)) // level+1 因为第一层是1，不是0
        }
    }

    /// 移除ANSI转义序列
    fn remove_ansi_codes(&self, text: &str) -> String {
        // 简单的ANSI转义序列移除
        let mut result = String::new();
        let mut chars = text.chars().peekable();
        
        while let Some(ch) = chars.next() {
            if ch == '\x1b' {
                // 跳过ANSI转义序列
                if chars.peek() == Some(&'[') {
                    chars.next(); // 跳过 '['
                    while let Some(c) = chars.next() {
                        if c.is_ascii_alphabetic() || c == '~' {
                            break;
                        }
                    }
                }
            } else {
                result.push(ch);
            }
        }
        result
    }

    /// 判断是否为文件
    fn is_file(&self, name: &str) -> bool {
        // 有扩展名的是文件
        if name.contains('.') && !name.starts_with('.') {
            if let Some(dot_pos) = name.rfind('.') {
                return dot_pos > 0 && dot_pos < name.len() - 1;
            }
        }
        
        // 常见的无扩展名文件
        matches!(name, "Cargo.lock" | "Dockerfile" | "Makefile" | "LICENSE" | "README" | "CHANGELOG")
    }
}

/// Excel生成器
struct ExcelGenerator;

impl ExcelGenerator {
    fn new() -> Self {
        Self
    }

    /// 生成Excel文件
    fn generate(&self, items: Vec<TreeItem>, output_path: &str) -> Result<()> {
        let mut workbook = Workbook::new();
        let worksheet = workbook.add_worksheet();

        // 转换为Excel行数据（先转换以获取max_level）
        let rows = self.convert_to_rows(items);
        let max_level = if rows.is_empty() { 1 } else { rows[0].max_level };

        // 设置标题和格式
        self.setup_worksheet(worksheet, max_level)?;

        // 写入数据
        self.write_data(worksheet, &rows)?;

        // 保存文件
        workbook.save(output_path)
            .with_context(|| format!("无法保存Excel文件: {}", output_path))?;

        Ok(())
    }

    /// 设置工作表
    fn setup_worksheet(&self, worksheet: &mut Worksheet, max_level: usize) -> Result<()> {
        let header_format = Format::new()
            .set_bold()
            .set_background_color("#4F81BD")
            .set_font_color("#FFFFFF")
            .set_border(rust_xlsxwriter::FormatBorder::Thin);

        // 动态生成表头
        let mut col = 0;
        
        // 层级列：L1, L2, L3, ...
        for level in 1..=max_level {
            let header = format!("L{}", level);
            worksheet.write_with_format(0, col as u16, &header, &header_format)?;
            worksheet.set_column_width(col as u16, 20.0)?;  // 层级列宽度
            col += 1;
        }
        
                    // 完整路径列
            worksheet.write_with_format(0, col as u16, "完整路径", &header_format)?;
            worksheet.set_column_width(col as u16, 60.0)?;  // 增加宽度以适应长路径和统计信息
        col += 1;
        
        // 备注列
        worksheet.write_with_format(0, col as u16, "备注", &header_format)?;
        worksheet.set_column_width(col as u16, 30.0)?;

        Ok(())
    }

    /// 将TreeItem转换为ExcelRow
    fn convert_to_rows(&self, items: Vec<TreeItem>) -> Vec<ExcelRow> {
        let mut rows = Vec::new();
        let mut path_stack: Vec<String> = Vec::new();
        
        // 首先找出最大层级深度
        let max_level = items.iter()
            .filter(|item| !item.name.starts_with("📊"))
            .map(|item| item.level)
            .max()
            .unwrap_or(1);

        for item in items {
            // 统计信息特殊处理
            if item.name.starts_with("📊") {
                let mut levels = vec!["".to_string(); max_level];
                levels[0] = item.name.clone();
                
                rows.push(ExcelRow {
                    levels,
                    full_path: item.name.clone(),
                    max_level,
                    is_file: false,
                });
                continue;
            }

            // 调整路径栈到当前层级
            path_stack.truncate(item.level.saturating_sub(1));
            path_stack.push(item.name.clone());

            // 构建levels数组，填充到对应层级
            let mut levels = vec!["".to_string(); max_level];
            for (i, path_item) in path_stack.iter().enumerate() {
                if i < max_level {
                    levels[i] = path_item.clone();
                }
            }

            rows.push(ExcelRow {
                levels,
                full_path: item.full_path.clone(),
                max_level,
                is_file: item.is_file,
            });
        }

        rows
    }

    /// 写入Excel数据（支持层级合并单元格）
    fn write_data(&self, worksheet: &mut Worksheet, rows: &[ExcelRow]) -> Result<()> {
        if rows.is_empty() {
            return Ok(());
        }

        let max_level = rows[0].max_level;
        
        // 格式定义
        let dir_format = Format::new()
            .set_background_color("#E8F4FD")
            .set_border(rust_xlsxwriter::FormatBorder::Thin)
            .set_bold()
            .set_align(rust_xlsxwriter::FormatAlign::Center)
            .set_align(rust_xlsxwriter::FormatAlign::VerticalCenter);

        let file_format = Format::new()
            .set_background_color("#F0F8E8")
            .set_border(rust_xlsxwriter::FormatBorder::Thin);

        let path_format = Format::new()
            .set_background_color("#FFFEF7")
            .set_border(rust_xlsxwriter::FormatBorder::Thin);

        let notes_format = Format::new()
            .set_background_color("#F5F5F5")
            .set_border(rust_xlsxwriter::FormatBorder::Thin);

        let stats_format = Format::new()
            .set_background_color("#FFE4E1")
            .set_border(rust_xlsxwriter::FormatBorder::Thin)
            .set_bold()
            .set_font_color("#8B0000");

        let mut current_row = 1u32;

        // 分离统计行和数据行
        let mut data_rows = Vec::new();
        let mut stats_rows = Vec::new();
        
        for row in rows {
            if row.levels[0].starts_with("📊") {
                stats_rows.push(row);
            } else {
                data_rows.push(row);
            }
        }

        // 写入数据行，实现层级合并单元格
        self.write_data_with_merging(worksheet, &data_rows, max_level, &dir_format, &file_format, &path_format, &notes_format, &mut current_row)?;

        // 记录stats行数量，避免所有权问题
        let stats_count = stats_rows.len();
        
        // 写入统计行
        for stats_row in stats_rows {
            let total_cols = max_level + 2;
            
            // 设置统计行行高为20
            worksheet.set_row_height(current_row, 20.0)?;
            
            worksheet.merge_range(
                current_row, 0,
                current_row, (total_cols - 1) as u16,
                &stats_row.levels[0],
                &stats_format
            )?;
            current_row += 1;
        }

        // 冻结首行
        let _ = worksheet.set_freeze_panes(1, 0);

        // 自动筛选
        if !data_rows.is_empty() {
            let total_cols = max_level + 2;
            worksheet.autofilter(0, 0, (data_rows.len() + stats_count) as u32, (total_cols - 1) as u16)?;
        }

        Ok(())
    }

    /// 写入数据并实现层级合并单元格
    fn write_data_with_merging(
        &self,
        worksheet: &mut Worksheet,
        rows: &[&ExcelRow],
        max_level: usize,
        dir_format: &Format,
        file_format: &Format,
        path_format: &Format,
        notes_format: &Format,
        current_row: &mut u32,
    ) -> Result<()> {
        if rows.is_empty() {
            return Ok(());
        }

        // 先写入所有单元格内容
        for (row_idx, row) in rows.iter().enumerate() {
            let row_num = *current_row + row_idx as u32;
            
            // 层级列：写入每个层级的内容
            for (level_idx, level_name) in row.levels.iter().enumerate() {
                if !level_name.is_empty() {
                    let format = if row.is_file && level_idx == row.levels.len() - 1 {
                        file_format
                    } else {
                        dir_format
                    };
                    worksheet.write_with_format(row_num, level_idx as u16, level_name, format)?;
                }
            }

            // 完整路径列
            let path_col = max_level as u16;
            worksheet.write_with_format(row_num, path_col, &row.full_path, path_format)?;

            // 备注列
            let notes_col = max_level as u16 + 1;
            worksheet.write_with_format(row_num, notes_col, "", notes_format)?;
        }

        // 然后实现合并单元格逻辑
        for level_idx in 0..max_level {
            self.merge_level_column(worksheet, rows, level_idx, *current_row, dir_format)?;
        }

        *current_row += rows.len() as u32;
        Ok(())
    }

    /// 合并指定层级列的单元格
    fn merge_level_column(
        &self,
        worksheet: &mut Worksheet,
        rows: &[&ExcelRow],
        level_idx: usize,
        start_row: u32,
        dir_format: &Format,
    ) -> Result<()> {
        let mut i = 0;
        while i < rows.len() {
            let current_value = &rows[i].levels[level_idx];
            
            // 跳过空值
            if current_value.is_empty() {
                i += 1;
                continue;
            }

            // 找到相同值的连续范围，考虑前面层级的约束
            let mut j = i + 1;
            while j < rows.len() {
                // 检查当前层级值是否相同
                if rows[j].levels[level_idx] != *current_value {
                    break;
                }
                
                // 检查前面的层级是否也相同（重要：确保是同一个父目录下）
                let mut same_parent = true;
                for prev_level in 0..level_idx {
                    if rows[i].levels[prev_level] != rows[j].levels[prev_level] {
                        same_parent = false;
                        break;
                    }
                }
                
                if !same_parent {
                    break;
                }
                
                j += 1;
            }

            // 如果有多行相同值，进行合并
            if j - i > 1 {
                let start_merge_row = start_row + i as u32;
                let end_merge_row = start_row + (j - 1) as u32;
                
                worksheet.merge_range(
                    start_merge_row, level_idx as u16,
                    end_merge_row, level_idx as u16,
                    current_value,
                    dir_format
                )?;
            }

            i = j;
        }

        Ok(())
    }
}

fn main() -> Result<()> {
    let matches = Command::new("tree-to-excel")
        .about("将tree命令输出转换为Excel表格，支持合并单元格层级展示")
        .version("1.0")
        .arg(
            Arg::new("input")
                .short('i')
                .long("input")
                .value_name("FILE")
                .help("输入文件路径（tree命令输出）")
        )
        .arg(
            Arg::new("output")
                .short('o')
                .long("output")
                .value_name("FILE")
                .help("输出Excel文件路径")
                .default_value("tree_output.xlsx")
        )
        .arg(
            Arg::new("include_hidden")
                .short('a')
                .long("include-hidden")
                .action(clap::ArgAction::SetTrue)
                .help("包含隐藏目录/文件（以.开头的项目，如.git）")
        )
        .get_matches();

    // 读取输入
    let input_content = if let Some(input_file) = matches.get_one::<String>("input") {
        println!("📖 读取tree输出文件: {}", input_file);
        fs::read_to_string(input_file)
            .with_context(|| format!("无法读取文件: {}", input_file))?
    } else {
        println!("📖 从标准输入读取tree输出（Ctrl+D结束）:");
        let mut buffer = String::new();
        io::stdin().read_to_string(&mut buffer)
            .context("无法从标准输入读取")?;
        buffer
    };

    let output_path = matches.get_one::<String>("output").unwrap();
    let include_hidden = matches.get_flag("include_hidden");

    if include_hidden {
        println!("🔄 解析tree结构（包含隐藏目录）...");
    } else {
        println!("🔄 解析tree结构（默认忽略.git等隐藏目录）...");
    }
    
    // 解析tree输出
    let parser = TreeParser::new();
    let items = parser.parse(&input_content, include_hidden)
        .context("解析tree输出失败")?;

    println!("📊 找到 {} 个文件/目录", items.len());

    // 生成Excel
    println!("📝 生成Excel文件: {}", output_path);
    let generator = ExcelGenerator::new();
    generator.generate(items, output_path)
        .context("生成Excel文件失败")?;

    println!("✅ 完成！Excel文件已保存");
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_line() {
        let parser = TreeParser::new();
        
        let test_cases = vec![
            ("├── src", Some((1, "src".to_string()))),
            ("│   ├── main.rs", Some((2, "main.rs".to_string()))),
            ("│   │   └── lib.rs", Some((3, "lib.rs".to_string()))),
        ];

        for (input, expected) in test_cases {
            let result = parser.parse_line(input);
            assert_eq!(result, expected, "Failed for input: {}", input);
        }
    }
}