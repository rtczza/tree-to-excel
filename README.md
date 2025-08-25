# Tree到Excel转换工具 - 终极版

一个精确的Rust命令行工具，将`tree`命令输出转换为带有合并单元格的Excel表格。

## ✨ 主要特性

✅ **完美解析**: 正确处理UTF-8编码的tree符号（├──、└──、│）  
✅ **动态列数**: 根据实际层级深度自动调整Excel列数（L1|L2|L3|...|完整路径|备注）  
✅ **层级合并单元格**: 相同父目录下的项目在每个层级列中智能合并显示，支持垂直居中对齐  
✅ **多层级支持**: 支持任意深度的目录层级关系  
✅ **完整路径**: 构建准确的完整文件路径（如`src/bin/aaabbb.rs`）  
✅ **隐藏目录过滤**: 默认过滤.git等隐藏目录，可选择包含（-a参数）  
✅ **统计信息**: 自动提取统计信息，与过滤逻辑保持一致  
✅ **备注列**: 提供空白备注列供用户自定义填写  

## 🚀 使用方法

### 基本使用

```bash
# 编译程序
cargo build --release

# 从文件转换（默认过滤.git等隐藏目录）
./target/release/tree-to-excel -i your_tree.txt -o output.xlsx

# 包含隐藏目录/文件（如.git、.gitignore等）
./target/release/tree-to-excel -i your_tree.txt -o output.xlsx -a

# 从标准输入转换
tree /path/to/project | ./target/release/tree-to-excel -o project_structure.xlsx
```

### 命令行参数

```bash
tree-to-excel [OPTIONS]

OPTIONS:
    -i, --input <FILE>     输入文件路径（tree命令输出）
    -o, --output <FILE>    输出Excel文件路径 [默认: tree_output.xlsx]
    -a, --include-hidden   包含隐藏目录/文件（以.开头的项目，如.git）
    -h, --help             显示帮助信息
    -V, --version          显示版本信息
```

## 📊 输出Excel格式

生成的Excel文件使用**动态列数**，根据实际层级深度自动调整：

### 3层结构示例（带合并单元格）：
| L1 | L2 | L3 | 完整路径 | 备注 |
|----|----|----|----------|------|
| **src**<br/>**（合并）** | **bin**<br/>**（合并）** | aaabbb.rs | src/bin/aaabbb.rs | (空列供填写) |
| ↑ | ↑ | aaaccc.rs | src/bin/aaaccc.rs | (空列供填写) |
| ↑ | **commands**<br/>**（合并）** | add.rs | src/commands/add.rs | (空列供填写) |
| ↑ | ↑ | delete.rs | src/commands/delete.rs | (空列供填写) |

**合并效果说明**：
- **L1列**：所有src下的项目在L1列中合并显示"src"
- **L2列**：bin目录下的文件在L2列中合并显示"bin"，commands目录下的文件合并显示"commands"
- **L3列**：每个具体文件名单独显示

## 📊 统计信息处理

### 智能统计逻辑
程序会根据过滤设置自动调整统计信息：

- **默认模式（过滤隐藏目录）**：重新计算实际解析的文件和目录数量
- **包含隐藏目录模式（-a参数）**：使用tree命令的原始统计信息

### 统计示例对比
```bash
# 原始tree输出统计（包含.git等隐藏目录）
69 directories, 116 files

# 过滤隐藏目录后的准确统计
15 directories, 45 files
```

这确保了Excel中显示的统计信息与实际内容完全一致。

### 视觉效果

- 🔵 **目录**: 浅蓝色背景，加粗字体  
- 🟢 **文件**: 浅绿色背景，普通字体  
- 🟡 **路径列**: 淡黄色背景，显示完整路径  
- 📝 **备注列**: 淡灰色背景，空列供用户自定义填写  
- 🌸 **统计行**: 粉色背景，跨所有列显示统计信息

## 🔧 技术实现

- **语言**: Rust 2021 Edition
- **Excel处理**: rust_xlsxwriter crate
- **UTF-8支持**: 原生Unicode字符处理
- **解析算法**: 基于状态机的层级解析
- **错误处理**: anyhow统一错误管理

---

**这是一个完全重写、精心优化的版本，专门解决UTF-8编码、层级解析、路径构建和Excel展示的所有问题。**