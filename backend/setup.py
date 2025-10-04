"""Setup configuration for openagent-terminal backend."""

from setuptools import setup, find_packages

setup(
    name="openagent-terminal",
    version="0.1.0",
    description="Python backend for OpenAgent-Terminal (AI-native terminal emulator)",
    author="OpenAgent Team",
    author_email="",
    url="https://github.com/yourusername/openagent-terminal",
    packages=find_packages(),
    python_requires=">=3.9",
    install_requires=[
        "openagent>=0.1.3",  # Core agent framework
        "jsonrpcserver>=5.0",  # JSON-RPC server
        "asyncio-dgram>=2.1",  # Async Unix socket support
        "pydantic>=2.0",  # Data validation
    ],
    extras_require={
        "dev": [
            "pytest>=7.0",
            "pytest-asyncio>=0.21",
            "pytest-cov>=4.0",
            "black>=23.0",
            "mypy>=1.0",
            "ruff>=0.1",
        ],
    },
    entry_points={
        "console_scripts": [
            "openagent-terminal-bridge=openagent_terminal.bridge:main",
        ],
    },
    classifiers=[
        "Development Status :: 2 - Pre-Alpha",
        "Intended Audience :: Developers",
        "License :: OSI Approved :: MIT License",
        "Programming Language :: Python :: 3",
        "Programming Language :: Python :: 3.9",
        "Programming Language :: Python :: 3.10",
        "Programming Language :: Python :: 3.11",
    ],
)
