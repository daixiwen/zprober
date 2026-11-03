library ieee;
use ieee.std_logic_1164.all;
use ieee.numeric_std.all;
use ieee.math_real.all;

library fpgacapzero;
library neorv32;

entity toplevel is
    port (
        clk   : in std_logic;
        reset : in std_logic;

        -- GPIO (available if IO_GPIO_NUM > 0) --
        gpio_dir_o     : out std_ulogic_vector(31 downto 0);                     -- direction control (0=in, 1=out)
        gpio_o         : out std_ulogic_vector(31 downto 0);                     -- parallel output
        gpio_i         : in  std_ulogic_vector(31 downto 0) := (others => 'L');  -- parallel input; interrupt-capable

        -- Primary UART0 (available if IO_UART0_EN = true) --
        uart0_txd_o    : out std_ulogic;                                         -- send data
        uart0_rxd_i    : in  std_ulogic := 'L';                                  -- receive data
        uart0_rtsn_o   : out std_ulogic;                                         -- HW flow control: RX ready to receive, low-active
        uart0_ctsn_i   : in  std_ulogic := 'L';                                  -- HW flow control: TX allowed to transmit, low-active

        -- JTAG pads
        tck_pad_i     : in  std_logic;
        tms_pad_i     : in  std_logic;
        tdi_pad_i     : in  std_logic;
        trstb_pad_i   : in  std_logic;
        tdo_pad_o     : out std_logic

        );
end entity toplevel;

architecture rtl of toplevel is

    signal probe_in : std_logic_vector(7 downto 0);

begin

    neorv32_top_inst: entity neorv32.neorv32_top
        generic map(
            CLOCK_FREQUENCY => 100000000,
            IO_GPIO_NUM => 32,
            IO_GPIO_DIR_EN => true,
            IO_UART0_RX_FIFO => 16,
            IO_UART0_TX_FIFO => 16
        )
        port map(
            clk_i => clk,
            rstn_i => not reset,
            gpio_dir_o => gpio_dir_o,
            gpio_o => gpio_o,
            gpio_i => gpio_i,
            uart0_txd_o => uart0_txd_o,
            uart0_rxd_i => uart0_rxd_i,
            uart0_rtsn_o => uart0_rtsn_o,
            uart0_ctsn_i => uart0_ctsn_i
        );

    fcapz_ela_polarfire_inst: entity fpgacapzero.fcapz_ela_polarfire
        generic map(
            SAMPLE_W => 8,
            DEPTH => 1024,
            TRIG_STAGES => 1,
            STOR_QUAL => 0,
            INPUT_PIPE => 0,
            NUM_CHANNELS => 1,
            DECIM_EN => 0,
            EXT_TRIG_EN => 0,
            TIMESTAMP_W => 0,
            NUM_SEGMENTS => 1,
            PROBE_MUX_W => 0,
            STARTUP_ARM => 0,
            DEFAULT_TRIG_EXT => 0,
            BURST_W => 256,
            BURST_EN => 1,
            SINGLE_CHAIN_BURST => 1,
            IR_USER1 => x"20",
            IR_USER2 => x"21",
            EIO_EN => 0,
            EIO_IN_W => 1,
            EIO_OUT_W => 1,
            REL_COMPARE => 0,
            DUAL_COMPARE => 1,
            USER1_DATA_EN => 1,
            RLE_W => 0
        )
        port map(
            sample_clk => clk,
            sample_rst => reset,
            probe_in => probe_in,
            trigger_in => '0',
            trigger_out => open,
            armed_out => open,
            eio_probe_in => (others => '0'),
            eio_probe_out => open,
            tck_pad_i => tck_pad_i,
            tms_pad_i => tms_pad_i,
            tdi_pad_i => tdi_pad_i,
            trstb_pad_i => trstb_pad_i,
            tdo_pad_o => tdo_pad_o
        );

end architecture;