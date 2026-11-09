library ieee;
use ieee.std_logic_1164.all;
use ieee.numeric_std.all;
use ieee.math_real.all;

entity original is
    port (
        clk   : in std_logic;
        reset : in std_logic;
        
        data_in : in std_logic_vector(7 downto 0);
        data_out: out std_logic_vector(7 downto 0)
-- ZProber[[ DO NOT EDIT THE LINES BELOW
;
    debug : out std_logic_vector(7 downto 0)
-- ZProber]]
    );
end entity original;

architecture rtl of original is

    signal data_internal : std_logic_vector(7 downto 0);

-- ZProber[[ DO NOT EDIT THE LINES BELOW
    signal extra_signal : std_logic_vector(31 downto 0);
-- ZProber]]
begin
-- ZProber[[ DO NOT EDIT THE LINES BELOW
    extra_signal <= data_internal;
-- ZProber]]

    i_component_0 : entity work.my_component
        generic map (
            const_a => 3,
            const_b => 4
        )
        port map(
            clk => clk,
            reset => reset,
            data_in => data_in,
            data_out => extra_signal
        );

    i_component_1 : entity work.my_other_component
        generic map (
            bugs => false
        )
        port map(
            clk => clk,
            reset => reset,
            data_in => data_internal,
            data_out => data_out
-- ZProber[[ DO NOT EDIT THE LINES BELOW
,
    debug => debug
-- ZProber]]
        );

end architecture;
